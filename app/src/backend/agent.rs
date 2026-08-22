use super::*;
use capability::{CapabilityQuery, CapabilityReply};
use gateway::{CredentialKind, GatewayQuery, GatewayReply};
use identity::{AccountView, IdentityQuery, IdentityReply};
use iced::advanced::widget::{Operation, Tree, tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, renderer};
use iced::futures::SinkExt as _;
use iced::{Element, Length, Rectangle, Size, Subscription, Theme, mouse};
use ui_lang_components::ui::terminal;
use saga::{SagaQuery, SagaReply, SagaStatus, SagaView};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use tokio_tungstenite::tungstenite::Message;

const AGENT_CONTEXT_ROWS: usize = 32;
const AGENT_CONTEXT_BYTES: usize = 48 * 1024;
const LINK_TOKEN_BYTES: u64 = 4 * 1024;
const MAX_ACTIVITY_ROWS: usize = 32;
const RUNNER_RESULT_VERSION: u64 = 1;
/// The unpicked host: the run executes on the node the app is connected to.
/// `app/src/ui/state/shell.ice` seeds the picker with this exact string, which
/// a test below pins.
const LOCAL_HOST_NODE: &str = "This node";

/// The app's handle on a `ducktape agent pty` session. The pty engine, its grid
/// and the widget that draws it are `ui_lang_components::ui::terminal`; this is
/// the Ice-facing wrapper, which exists so the extern type keeps its name and
/// the app decides what a notice means.
#[derive(Clone)]
pub struct AgentTerminalSession(terminal::Session);

impl Hash for AgentTerminalSession {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentTerminalNotice {
    pub running: bool,
    pub title: String,
}

#[derive(Clone)]
pub struct AgentTerminalStarted {
    pub session: AgentTerminalSession,
    pub title: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentCredential {
    pub name: String,
    pub provider: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentCredentialsData {
    pub generation: i64,
    pub rows: Vec<AgentCredential>,
}

/// One peer that announced a provider this app can launch: the 64-hex node key
/// the CLI's `--host-node` takes, the display name identity knows for the
/// account operating it (a short key when identity knows none), and the tags
/// the node announced.
#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentHostNode {
    pub key: String,
    pub label: String,
    pub providers: Vec<String>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentHostNodesData {
    pub generation: i64,
    pub rows: Vec<AgentHostNode>,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentChatEntry {
    pub id: i64,
    pub role: String,
    pub body: String,
    pub provider: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentActivity {
    pub id: i64,
    pub title: String,
    pub detail: String,
    pub status: String,
}

#[derive(Clone, Debug, Hash, PartialEq)]
pub struct AgentChatEvent {
    pub id: i64,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub status: String,
    pub answer: String,
    pub saga_id: String,
}

/// The current compute-service result contract. Shell renders only the message
/// facet, but still names every top-level field so contract drift is rejected
/// instead of leaking an unfamiliar machine envelope into the conversation.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AgentRunnerResult {
    ducktape_runner_result: u64,
    response_text: String,
    #[serde(rename = "workspace_receipt")]
    _workspace_receipt: serde_json::Value,
    #[serde(default, rename = "sink")]
    _sink: Option<serde_json::Value>,
    #[serde(default, rename = "status")]
    _status: Option<serde_json::Value>,
}

pub fn idle_agent_terminal() -> AgentTerminalSession {
    AgentTerminalSession(terminal::idle_session())
}

pub async fn start_agent_terminal(
    rpc: String,
    provider: String,
    credential: String,
    host_node: String,
) -> Result<AgentTerminalStarted, AppError> {
    let provider = agent_provider(&provider).map_err(AppError::from)?;
    let args = agent_pty_args(&rpc, provider, &credential, &host_node);
    let program = ducktape_binary();
    let working_directory =
        std::env::current_dir().map_err(|error| AppError::from(error.to_string()))?;
    let title = format!("{} · raw session", provider_title(provider));
    let session = terminal::spawn_session(program, args, working_directory, title.clone())
        .map_err(|error| AppError::from(error.message))?;

    Ok(AgentTerminalStarted {
        session: AgentTerminalSession(session),
        title,
    })
}

pub fn agent_terminal_events(session: AgentTerminalSession) -> Subscription<AgentTerminalNotice> {
    terminal::terminal_events(session.0).map(agent_terminal_notice)
}

pub fn agent_terminal_surface(session: &AgentTerminalSession) -> Element<'static, ()> {
    terminal::terminal_surface(&session.0)
}

/// Parsed Markdown whose items live with the widget instead of borrowing an
/// Ice state field. The app is multi-window, so the native Ice markdown node
/// cannot ask for a theme without a window id; this adapter owns the parse and
/// uses the app palette's stable text roles directly.
pub fn agent_markdown(source: String, dark: bool) -> Element<'static, String> {
    Element::new(AgentMarkdown::new(&source, dark))
}

/// The forge reader's Markdown: the same surface as [`agent_markdown`], plus
/// the document's in-repo pictures (parked by `forge_blob` under `doc`)
/// drawn in place of their alt text.
pub fn forge_markdown(source: String, doc: String, dark: bool) -> Element<'static, String> {
    Element::new(AgentMarkdown::new(&source, dark).with_doc(doc))
}

pub fn focus_agent_terminal(session: AgentTerminalSession) -> iced::Task<()> {
    terminal::focus_terminal(session.0)
}

pub async fn load_agent_credentials(
    rpc: String,
    generation: i64,
) -> Result<AgentCredentialsData, HydrationError> {
    let result = async {
        let client = rpc_client(&rpc)?;
        let reply: GatewayReply = client
            .query("gateway", &GatewayQuery::Credentials {})
            .await
            .map_err(String::from)?;
        let GatewayReply::Credentials(records) = reply else {
            return Err("the gateway returned the wrong credential reply".into());
        };
        let mut rows = records
            .into_iter()
            .map(|record| AgentCredential {
                name: record.name,
                provider: credential_provider(record.kind).into(),
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.provider
                .cmp(&right.provider)
                .then_with(|| left.name.cmp(&right.name))
        });
        Ok(AgentCredentialsData { generation, rows })
    }
    .await;
    result.map_err(|cause: String| HydrationError {
        generation,
        message: user_error(cause),
    })
}

pub fn agent_credential_names(rows: Vec<AgentCredential>, provider: String) -> Vec<String> {
    rows.into_iter()
        .filter(|row| row.provider == provider)
        .map(|row| row.name)
        .collect()
}

pub fn agent_credential_choice(
    rows: Vec<AgentCredential>,
    provider: String,
    current: String,
) -> String {
    let names = agent_credential_names(rows, provider);
    if names.iter().any(|name| name == &current) {
        current
    } else {
        names.into_iter().next().unwrap_or_default()
    }
}

/// The COMPUTE band's other half: WHICH peer can run the work. The capability
/// registry is the network's node -> announced-tags map, and identity names the
/// account each announcing node is bound to.
pub async fn load_agent_host_nodes(
    rpc: String,
    generation: i64,
) -> Result<AgentHostNodesData, HydrationError> {
    let result = async {
        let client = rpc_client(&rpc)?;
        let reply: CapabilityReply = client
            .query("capability", &CapabilityQuery::All)
            .await
            .map_err(String::from)?;
        let CapabilityReply::All(registry) = reply else {
            return Err("the capability registry returned the wrong reply".into());
        };
        let reply: IdentityReply = client
            .query(
                "identity",
                &IdentityQuery::All {
                    from: 0,
                    limit: u64::MAX,
                },
            )
            .await
            .map_err(String::from)?;
        let IdentityReply::Accounts(accounts) = reply else {
            return Err("the identity module returned the wrong reply".into());
        };
        let names = node_account_names(&accounts);
        let mut rows = registry
            .into_iter()
            .filter_map(|(node, tags)| host_node_row(&names, &node, tags))
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| left.label.cmp(&right.label));
        Ok(AgentHostNodesData { generation, rows })
    }
    .await;
    result.map_err(|cause: String| HydrationError {
        generation,
        message: user_error(cause),
    })
}

/// The picker's rows: the local default first, then one row per announcing
/// peer. The empty list is still one row, so "this node" is always reachable.
pub fn agent_host_node_options(rows: Vec<AgentHostNode>) -> Vec<String> {
    std::iter::once(LOCAL_HOST_NODE.to_string())
        .chain(rows.iter().map(host_node_option))
        .collect()
}

/// The `--host-node` value behind a picked row. The local default — and any
/// label the registry no longer carries — resolves to the empty string, which
/// the argv builders spell as no flag at all.
pub fn agent_host_node_key(rows: Vec<AgentHostNode>, option: String) -> String {
    rows.iter()
        .find(|row| host_node_option(row) == option)
        .map(|row| row.key.clone())
        .unwrap_or_default()
}

/// The ONE spelling of a picker row — who runs it, and what it announced — so
/// the option list and the reverse lookup above cannot drift apart.
fn host_node_option(row: &AgentHostNode) -> String {
    format!("{} · {}", row.label, row.providers.join(", "))
}

/// node key -> the display name of the account that bound it. Accounts without
/// a name contribute nothing, so their nodes fall back to a short key.
fn node_account_names(accounts: &[AccountView]) -> HashMap<Vec<u8>, String> {
    accounts
        .iter()
        .filter_map(|account| Some((account, account.display_name.clone()?)))
        .flat_map(|(account, name)| {
            account
                .nodes
                .iter()
                .map(move |node| (node.node_key.clone(), name.clone()))
        })
        .collect()
}

/// A registry row becomes a pick only when it announces a provider this app can
/// launch — the same provider set the argv builders accept, so the picker can
/// never offer a host the run would bounce off.
fn host_node_row(
    names: &HashMap<Vec<u8>, String>,
    node: &[u8],
    tags: Vec<String>,
) -> Option<AgentHostNode> {
    let providers = tags
        .into_iter()
        .filter(|tag| agent_provider(tag).is_ok())
        .collect::<Vec<_>>();
    if providers.is_empty() {
        return None;
    }
    let key = hex_encode(node);
    let label = names
        .get(node)
        .cloned()
        .unwrap_or_else(|| short_label(&key));
    Some(AgentHostNode {
        key,
        label,
        providers,
    })
}

pub fn agent_provider_label(provider: String) -> String {
    provider_title(&provider).into()
}

pub fn agent_provider_initial(provider: String) -> String {
    match provider.as_str() {
        "codex" => "C".into(),
        "claude" => "A".into(),
        _ => "?".into(),
    }
}

pub fn agent_credential_caption(provider: String, credential: String) -> String {
    let credential = credential.trim();
    if credential.is_empty() {
        format!(
            "Runs on {} · no credential selected",
            provider_title(&provider)
        )
    } else {
        format!("Runs on {} · {credential}", provider_title(&provider))
    }
}

pub fn agent_register_hint(provider: String) -> String {
    format!("Register one with `ducktape user cred add {provider}`")
}

pub fn agent_composer_hint(provider: String) -> String {
    format!("Message {}…", provider_title(&provider))
}

pub fn agent_chat_push_user(
    mut entries: Vec<AgentChatEntry>,
    body: String,
    provider: String,
) -> Vec<AgentChatEntry> {
    entries.push(AgentChatEntry {
        id: next_chat_entry_id(&entries),
        role: "user".into(),
        body: body.trim().to_string(),
        provider,
    });
    entries
}

pub fn agent_chat_finish(
    mut entries: Vec<AgentChatEntry>,
    body: String,
    provider: String,
) -> Vec<AgentChatEntry> {
    entries.push(AgentChatEntry {
        id: next_chat_entry_id(&entries),
        role: "assistant".into(),
        body,
        provider,
    });
    entries
}

pub fn agent_activity_apply(
    mut rows: Vec<AgentActivity>,
    event: AgentChatEvent,
) -> Vec<AgentActivity> {
    let is_activity = event.kind == "activity";
    if !is_activity {
        return rows;
    }
    if let Some(existing) = rows.iter_mut().find(|row| row.title == event.title) {
        existing.detail = event.detail;
        existing.status = event.status;
        return rows;
    }
    rows.push(AgentActivity {
        id: event.id,
        title: event.title,
        detail: event.detail,
        status: event.status,
    });
    if rows.len() > MAX_ACTIVITY_ROWS {
        rows.drain(..rows.len() - MAX_ACTIVITY_ROWS);
    }
    rows
}

pub fn agent_event_status(current: String, event: AgentChatEvent) -> String {
    match event.kind.as_str() {
        "status" => event.title,
        "answer" | "error" => String::new(),
        _ => current,
    }
}

pub fn agent_event_detail(current: String, event: AgentChatEvent) -> String {
    match event.kind.as_str() {
        "status" => event.detail,
        "answer" | "error" => String::new(),
        _ => current,
    }
}

pub fn agent_event_saga(current: String, event: AgentChatEvent) -> String {
    if event.saga_id.is_empty() {
        current
    } else {
        event.saga_id
    }
}

pub fn agent_event_live(current: String, event: AgentChatEvent) -> String {
    match event.kind.as_str() {
        "preview" => event.answer,
        "answer" | "error" => String::new(),
        _ => current,
    }
}

pub fn agent_event_error(current: String, event: AgentChatEvent) -> String {
    match event.kind.as_str() {
        "error" => event.answer,
        "answer" => String::new(),
        _ => current,
    }
}

pub fn agent_event_busy(event: AgentChatEvent) -> bool {
    !matches!(event.kind.as_str(), "answer" | "error")
}

pub fn agent_event_entries(
    entries: Vec<AgentChatEntry>,
    event: AgentChatEvent,
    provider: String,
) -> Vec<AgentChatEntry> {
    if event.kind != "answer" {
        return entries;
    }
    agent_chat_finish(entries, event.answer, provider)
}

pub fn agent_chat_prompt(entries: Vec<AgentChatEntry>) -> String {
    let start = entries.len().saturating_sub(AGENT_CONTEXT_ROWS);
    let mut kept = Vec::new();
    let mut used = 0usize;
    for entry in entries[start..].iter().rev() {
        let label = if entry.role == "assistant" {
            "Assistant"
        } else {
            "User"
        };
        let row = format!("{label}: {}", entry.body.trim());
        let next_bytes = used.saturating_add(row.len() + 2);
        if !kept.is_empty() && next_bytes > AGENT_CONTEXT_BYTES {
            break;
        }
        used = next_bytes;
        kept.push(row);
    }
    kept.reverse();
    let transcript = kept.join("\n\n");
    format!(
        "Continue this conversation. Answer the final User message directly. \
         Use prior turns only as context and do not repeat the transcript.\n\n{transcript}"
    )
}

pub fn agent_chat_turn(
    rpc: String,
    provider: String,
    credential: String,
    host_node: String,
    entries: Vec<AgentChatEntry>,
) -> iced::futures::stream::BoxStream<'static, AgentChatEvent> {
    let (sender, receiver) = tokio::sync::mpsc::channel(64);
    tokio::spawn(async move {
        run_agent_chat(sender, rpc, provider, credential, host_node, entries).await;
    });
    iced::futures::stream::unfold(receiver, |mut receiver| async move {
        receiver.recv().await.map(|event| (event, receiver))
    })
    .boxed()
}

async fn run_agent_chat(
    sender: tokio::sync::mpsc::Sender<AgentChatEvent>,
    rpc: String,
    provider: String,
    credential: String,
    host_node: String,
    entries: Vec<AgentChatEntry>,
) {
    let result = run_agent_chat_inner(&sender, rpc, provider, credential, host_node, entries).await;
    if let Err(message) = result {
        let _ = sender.send(chat_error(message)).await;
    }
}

async fn run_agent_chat_inner(
    sender: &tokio::sync::mpsc::Sender<AgentChatEvent>,
    rpc: String,
    provider: String,
    credential: String,
    host_node: String,
    entries: Vec<AgentChatEntry>,
) -> Result<(), String> {
    let provider = agent_provider(&provider)?;
    let credential = credential.trim();
    if credential.is_empty() {
        return Err("Choose a registered credential before sending a message.".into());
    }
    let prompt = agent_chat_prompt(entries);
    sender
        .send(chat_status("Scheduling", "Submitting a durable agent run"))
        .await
        .map_err(|_| "the chat view closed".to_string())?;
    let output = tokio::process::Command::new(ducktape_binary())
        .args(agent_sched_args(
            &rpc, provider, credential, &host_node, &prompt,
        ))
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|error| format!("could not run the ducktape agent command: {error}"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if message.is_empty() {
            format!("the agent command exited with {}", output.status)
        } else {
            clip_text(&message, 2_000)
        });
    }
    let stdout = String::from_utf8(output.stdout)
        .map_err(|_| "the agent command returned a non-UTF-8 run id".to_string())?;
    let saga_id = stdout
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(str::trim)
        .ok_or_else(|| "the agent command returned no run id".to_string())?
        .to_string();
    let dispatch_id = dispatch_id_from_saga(&saga_id)?;
    sender
        .send(AgentChatEvent {
            id: 1,
            kind: "status".into(),
            title: format!("Running {}", provider_title(provider)),
            detail: host_node_detail(&host_node),
            status: String::new(),
            answer: String::new(),
            saga_id: saga_id.clone(),
        })
        .await
        .map_err(|_| "the chat view closed".to_string())?;

    let (_, workspace) = workspace_at(&rpc).ok_or_else(|| {
        "This node has no matching local workspace, so its agent output cannot be opened."
            .to_string()
    })?;
    let token = read_link_token(&workspace)?;
    let (mut socket, _) = tokio_tungstenite::connect_async(agent_ws_url(&rpc))
        .await
        .map_err(|error| format!("could not open the node event stream: {error}"))?;
    let output_topic = format!("run-output:{dispatch_id}");
    let subscribe = serde_json::json!({
        "op": "subscribe",
        "topics": [output_topic, "module:saga"],
        "token": token,
    });
    socket
        .send(Message::Text(subscribe.to_string()))
        .await
        .map_err(|error| format!("could not subscribe to the agent run: {error}"))?;

    if emit_terminal_saga(sender, &rpc, &saga_id).await? {
        return Ok(());
    }
    let mut event_id = 2i64;
    while let Some(frame) = socket.next().await {
        let frame = frame.map_err(|error| format!("the node event stream closed: {error}"))?;
        let Message::Text(text) = frame else {
            continue;
        };
        let value: serde_json::Value = match serde_json::from_str(text.as_ref()) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if let Some(detail) = subscription_refusal(&value) {
            return Err(detail);
        }
        let topic = value["topic"].as_str().unwrap_or_default();
        if topic == output_topic {
            let Some(line) = value["item"]["line"].as_str() else {
                continue;
            };
            if let Some(mut event) = provider_output_event(provider, line, event_id) {
                event.saga_id = saga_id.clone();
                event_id += 1;
                if sender.send(event).await.is_err() {
                    return Ok(());
                }
            }
            continue;
        }
        if topic == "module:saga" && emit_terminal_saga(sender, &rpc, &saga_id).await? {
            return Ok(());
        }
    }
    Err("The node event stream ended before the agent run finished.".into())
}

async fn emit_terminal_saga(
    sender: &tokio::sync::mpsc::Sender<AgentChatEvent>,
    rpc: &str,
    saga_id: &str,
) -> Result<bool, String> {
    let client = rpc_client(rpc)?;
    let reply: SagaReply = client
        .query(
            "saga",
            &SagaQuery::Get {
                saga_id: saga_id.to_string(),
            },
        )
        .await
        .map_err(String::from)?;
    let SagaReply::Saga(Some(view)) = reply else {
        return Ok(false);
    };
    let Some(event) = terminal_saga_event(view, saga_id) else {
        return Ok(false);
    };
    sender
        .send(event)
        .await
        .map_err(|_| "the chat view closed".to_string())?;
    Ok(true)
}

fn terminal_saga_event(view: SagaView, saga_id: &str) -> Option<AgentChatEvent> {
    let (kind, answer) = match view.status {
        SagaStatus::Pending => return None,
        SagaStatus::Done => {
            let bytes = view.result.unwrap_or_default();
            let answer = agent_response_text(&bytes)
                .unwrap_or_else(|_| "The agent returned a result this app could not read.".into());
            ("answer", answer)
        }
        SagaStatus::Failed => (
            "error",
            view.error.unwrap_or_else(|| "The agent run failed.".into()),
        ),
        SagaStatus::TimedOut => ("error", "The durable agent run timed out.".into()),
        SagaStatus::Cancelled => ("error", "The durable agent run was cancelled.".into()),
    };
    Some(AgentChatEvent {
        id: i64::MAX,
        kind: kind.into(),
        title: if kind == "answer" {
            "Done"
        } else {
            "Run failed"
        }
        .into(),
        detail: String::new(),
        status: String::new(),
        answer,
        saga_id: saga_id.into(),
    })
}

fn agent_response_text(bytes: &[u8]) -> Result<String, String> {
    let result: AgentRunnerResult = serde_json::from_slice(bytes)
        .map_err(|error| format!("the runner result is malformed: {error}"))?;
    if result.ducktape_runner_result != RUNNER_RESULT_VERSION {
        return Err(format!(
            "runner result version {} is not supported",
            result.ducktape_runner_result
        ));
    }
    Ok(result.response_text)
}

fn provider_output_event(provider: &str, line: &str, id: i64) -> Option<AgentChatEvent> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    if provider == "claude" && value["type"].as_str() == Some("result") {
        let answer = value["result"].as_str()?.to_string();
        return Some(chat_preview(id, answer));
    }
    let event_type = value["type"].as_str().unwrap_or_default();
    let item = &value["item"];
    let item_type = item["type"]
        .as_str()
        .or_else(|| item["item_type"].as_str())
        .unwrap_or_default();
    if item_type == "agent_message" {
        let answer = item["text"]
            .as_str()
            .or_else(|| item["message"].as_str())?
            .to_string();
        return Some(chat_preview(id, answer));
    }
    let completed = event_type.ends_with("completed");
    let status = if completed { "done" } else { "running" };
    let (title, detail) = match item_type {
        "reasoning" => (
            "Reasoning".to_string(),
            json_text(item.get("text").or_else(|| item.get("summary"))),
        ),
        "command_execution" => (
            "Command".to_string(),
            json_text(
                item.get("command")
                    .or_else(|| item.get("aggregated_output")),
            ),
        ),
        "mcp_tool_call" => {
            let server = item["server"].as_str().unwrap_or("tool");
            let tool = item["tool"].as_str().unwrap_or("call");
            (
                format!("{server} · {tool}"),
                json_text(item.get("arguments")),
            )
        }
        "web_search" => ("Web search".to_string(), json_text(item.get("query"))),
        _ => return None,
    };
    Some(AgentChatEvent {
        id,
        kind: "activity".into(),
        title,
        detail: clip_text(&detail, 1_200),
        status: status.into(),
        answer: String::new(),
        saga_id: String::new(),
    })
}

fn chat_status(title: &str, detail: &str) -> AgentChatEvent {
    AgentChatEvent {
        id: 0,
        kind: "status".into(),
        title: title.into(),
        detail: detail.into(),
        status: String::new(),
        answer: String::new(),
        saga_id: String::new(),
    }
}

fn chat_preview(id: i64, answer: String) -> AgentChatEvent {
    AgentChatEvent {
        id,
        kind: "preview".into(),
        title: "Writing the answer".into(),
        detail: String::new(),
        status: String::new(),
        answer,
        saga_id: String::new(),
    }
}

fn chat_error(message: String) -> AgentChatEvent {
    AgentChatEvent {
        id: i64::MAX,
        kind: "error".into(),
        title: "Run failed".into(),
        detail: String::new(),
        status: String::new(),
        answer: user_error(message),
        saga_id: String::new(),
    }
}

fn next_chat_entry_id(entries: &[AgentChatEntry]) -> i64 {
    entries.last().map_or(1, |entry| entry.id.saturating_add(1))
}

fn credential_provider(kind: CredentialKind) -> &'static str {
    match kind {
        CredentialKind::Claude => "claude",
        CredentialKind::Codex => "codex",
    }
}

fn agent_provider(provider: &str) -> Result<&'static str, String> {
    match provider.trim().to_ascii_lowercase().as_str() {
        "claude" => Ok("claude"),
        "codex" => Ok("codex"),
        _ => Err("Choose Claude or Codex as the compute provider.".into()),
    }
}

fn provider_title(provider: &str) -> &'static str {
    match provider {
        "claude" => "Claude Code",
        "codex" => "Codex",
        _ => "Agent",
    }
}

fn agent_pty_args(rpc: &str, provider: &str, credential: &str, host_node: &str) -> Vec<String> {
    let mut args = vec![
        "agent".into(),
        "pty".into(),
        provider.into(),
        "--node".into(),
        rpc.into(),
    ];
    if !credential.trim().is_empty() {
        args.push("--cred".into());
        args.push(credential.trim().into());
    }
    push_host_node(&mut args, host_node);
    args
}

fn agent_sched_args(
    rpc: &str,
    provider: &str,
    credential: &str,
    host_node: &str,
    prompt: &str,
) -> Vec<String> {
    let mut args = vec![
        "agent".into(),
        "sched".into(),
        provider.into(),
        "--cred".into(),
        credential.into(),
        "--node".into(),
        rpc.into(),
    ];
    push_host_node(&mut args, host_node);
    args.push("--".into());
    args.push(prompt.into());
    args
}

/// Where the durable run actually lives, said the way the operator picked it —
/// the dialled node by default, else the peer the `--host-node` key names.
fn host_node_detail(host_node: &str) -> String {
    let host_node = host_node.trim();
    if host_node.is_empty() {
        return "The run is pinned to this node and will survive reconnects.".into();
    }
    format!(
        "The run is pinned to {} and will survive reconnects.",
        short_label(host_node)
    )
}

/// `--node` is the node the CLI DIALS; `--host-node` is the peer that EXECUTES.
/// An unpicked host is spelled by the flag's absence — that is what keeps the
/// work on the dialled node, exactly as it ran before the picker existed.
fn push_host_node(args: &mut Vec<String>, host_node: &str) {
    let host_node = host_node.trim();
    if host_node.is_empty() {
        return;
    }
    args.push("--host-node".into());
    args.push(host_node.into());
}

fn dispatch_id_from_saga(saga_id: &str) -> Result<String, String> {
    let dispatch_id = saga_id
        .rsplit_once('\u{1f}')
        .map(|(_, dispatch_id)| dispatch_id)
        .ok_or_else(|| "the agent returned a run id without a dispatch id".to_string())?;
    let valid = dispatch_id.len() == 64 && dispatch_id.chars().all(|c| c.is_ascii_hexdigit());
    if !valid {
        return Err("the agent returned a malformed dispatch id".into());
    }
    Ok(dispatch_id.to_ascii_lowercase())
}

fn agent_ws_url(rpc: &str) -> String {
    let base = if let Some(rest) = rpc.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = rpc.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        rpc.to_string()
    };
    format!("{}/v1/ws", base.trim_end_matches('/'))
}

fn read_link_token(workspace: &Path) -> Result<String, String> {
    let path = workspace.join("service-link.token");
    let metadata = std::fs::metadata(&path).map_err(|_| {
        "The node's agent event token is not available in its workspace.".to_string()
    })?;
    if metadata.len() > LINK_TOKEN_BYTES {
        return Err("The node's agent event token is unexpectedly large.".into());
    }
    let token = std::fs::read_to_string(path)
        .map_err(|_| "The node's agent event token could not be read.".to_string())?;
    let token = token.trim().to_string();
    if token.is_empty() {
        return Err("The node's agent event token is empty.".into());
    }
    Ok(token)
}

fn subscription_refusal(value: &serde_json::Value) -> Option<String> {
    let is_refusal =
        value["type"].as_str() == Some("refused") || value["type"].as_str() == Some("error");
    if !is_refusal {
        return None;
    }
    Some(
        value["detail"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .unwrap_or("The node refused the agent event stream.")
            .to_string(),
    )
}

fn json_text(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return String::new();
    };
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(items) => items
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>()
            .join(" "),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn clip_text(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        return text.to_string();
    }
    let end = text
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= limit)
        .last()
        .unwrap_or(0);
    format!("{}…", &text[..end])
}

/// The component reports `attention` as well; the app's Ice surface carries a
/// running flag and a title, so this is where the pty's bell is dropped. A
/// session that stopped running keeps the component's closing title.
fn agent_terminal_notice(notice: terminal::Notice) -> AgentTerminalNotice {
    AgentTerminalNotice {
        running: notice.running,
        title: notice.title,
    }
}


struct AgentMarkdown {
    items: Rc<[iced::widget::markdown::Item]>,
    settings: iced::widget::markdown::Settings,
    viewer: SelectViewer,
}

impl AgentMarkdown {
    fn new(source: &str, dark: bool) -> Self {
        use iced::widget::markdown;
        let link = if dark {
            iced::Color::from_rgb8(0xc9, 0x8a, 0x63)
        } else {
            iced::Color::from_rgb8(0xa0, 0x5a, 0x3c)
        };
        let code_background = if dark {
            iced::Color::from_rgb8(0x26, 0x25, 0x23)
        } else {
            iced::Color::from_rgb8(0xf3, 0xf2, 0xef)
        };
        let code_foreground = if dark {
            iced::Color::from_rgb8(0xe8, 0xe6, 0xdf)
        } else {
            iced::Color::from_rgb8(0x3f, 0x3e, 0x39)
        };
        let mono = iced::Font::with_name("Geist Mono");
        let style = markdown::Style {
            font: iced::Font::with_name("Geist"),
            inline_code_highlight: markdown::Highlight {
                background: code_background.into(),
                border: iced::border::rounded(4),
            },
            inline_code_padding: iced::Padding::from([1.0, 2.0]),
            inline_code_color: code_foreground,
            inline_code_font: mono,
            code_block_font: mono,
            link_color: link,
        };
        let mut settings = markdown::Settings::with_text_size(13.5, style);
        settings.h1_size = 18.0.into();
        settings.h2_size = 16.5.into();
        settings.h3_size = 15.0.into();
        settings.h4_size = 14.25.into();
        settings.h5_size = 13.5.into();
        settings.h6_size = 12.75.into();
        settings.code_size = 12.0.into();
        settings.spacing = 9.0.into();
        Self {
            items: markdown::parse(source).collect::<Vec<_>>().into(),
            settings,
            viewer: SelectViewer { doc: None },
        }
    }

    /// Draw the in-repo pictures parked under `doc` (see `picture.rs`).
    fn with_doc(mut self, doc: String) -> Self {
        self.viewer.doc = Some(doc);
        self
    }

    fn view(&self) -> Element<'_, String> {
        iced::widget::markdown::view_with(self.items.iter(), self.settings, &self.viewer)
    }
}

/// The Markdown surface with draggable text: every paragraph and heading
/// behind a [`SelectRich`] (one block is one selection, like the app's plain
/// Ice `text`), every code block one [`CodeSelect`] so a drag runs across
/// its lines. Lists, quotes and tables keep iced's default look and route
/// their text back through here. An image draws the picture `forge_blob`
/// parked under `doc` for its URL as written (relative path, `duck://files`,
/// `duck://forge/.../blob/...`), and keeps iced's default (the alt text in a
/// plate) for everything else — including every image when there is no
/// document, as in the agent's answers.
struct SelectViewer {
    doc: Option<String>,
}

impl<'a> iced::widget::markdown::Viewer<'a, String> for SelectViewer {
    fn on_link_click(url: iced::widget::markdown::Uri) -> String {
        url
    }

    fn image(
        &self,
        settings: iced::widget::markdown::Settings,
        url: &'a iced::widget::markdown::Uri,
        _title: &'a str,
        alt: &iced::widget::markdown::Text,
    ) -> Element<'a, String> {
        use iced::widget::{container, image, rich_text};
        let parked = self
            .doc
            .as_deref()
            .and_then(|doc| super::picture::inline_picture(doc, url));
        let Some(picture) = parked else {
            return container(
                rich_text(alt.spans(settings.style)).on_link_click(Self::on_link_click),
            )
            .padding(settings.spacing.0)
            .class(<iced::Theme as iced::widget::markdown::Catalog>::code_block())
            .into();
        };
        container(
            image(picture.handle)
                .width(Length::Fill)
                .height(Length::Shrink)
                .content_fit(iced::ContentFit::Contain),
        )
        .width(Length::Fill)
        .into()
    }

    fn heading(
        &self,
        settings: iced::widget::markdown::Settings,
        level: &'a iced::widget::markdown::HeadingLevel,
        text: &'a iced::widget::markdown::Text,
        index: usize,
    ) -> Element<'a, String> {
        use iced::widget::markdown::HeadingLevel;
        use iced::widget::{container, rich_text};
        let size = match level {
            HeadingLevel::H1 => settings.h1_size,
            HeadingLevel::H2 => settings.h2_size,
            HeadingLevel::H3 => settings.h3_size,
            HeadingLevel::H4 => settings.h4_size,
            HeadingLevel::H5 => settings.h5_size,
            HeadingLevel::H6 => settings.h6_size,
        };
        let spans = text.spans(settings.style);
        let rich = rich_text(spans.clone())
            .on_link_click(Self::on_link_click)
            .size(size);
        let top = match index > 0 {
            true => settings.text_size / 2.0,
            false => iced::Pixels::ZERO,
        };
        container(SelectRich::new(rich, spans, size))
            .padding(iced::padding::top(top))
            .into()
    }

    fn paragraph(
        &self,
        settings: iced::widget::markdown::Settings,
        text: &iced::widget::markdown::Text,
    ) -> Element<'a, String> {
        let spans = text.spans(settings.style);
        let rich = iced::widget::rich_text(spans.clone())
            .on_link_click(Self::on_link_click)
            .size(settings.text_size);
        SelectRich::new(rich, spans, settings.text_size).into()
    }

    fn code_block(
        &self,
        settings: iced::widget::markdown::Settings,
        _language: Option<&'a str>,
        _code: &'a str,
        lines: &'a [iced::widget::markdown::Text],
    ) -> Element<'a, String> {
        use iced::widget::markdown::Catalog as _;
        use iced::widget::{container, scrollable};
        let metrics = CodeMetrics {
            size: settings.code_size,
            line_height: iced::advanced::text::LineHeight::default(),
            font: settings.style.code_block_font,
        };
        let plate = CodeSelect::new(
            lines.iter().map(|line| line.spans(settings.style)),
            metrics,
            None,
            Length::Shrink,
        );
        container(
            scrollable(container(plate).padding(settings.code_size)).direction(
                scrollable::Direction::Horizontal(
                    scrollable::Scrollbar::default()
                        .width(settings.code_size / 2)
                        .scroller_width(settings.code_size / 2),
                ),
            ),
        )
        .width(Length::Fill)
        .padding(settings.code_size / 4)
        .class(Theme::code_block())
        .into()
    }
}

impl Widget<String, Theme, iced::Renderer> for AgentMarkdown {
    fn tag(&self) -> tree::Tag {
        self.view().as_widget().tag()
    }

    fn state(&self) -> tree::State {
        self.view().as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.view().as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.view().as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.view().as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.view().as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.view().as_widget_mut().layout(tree, renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        self.view()
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &iced::Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, String>,
        viewport: &Rectangle,
    ) {
        self.view().as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.view()
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.view()
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }
}



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_args_use_the_real_agent_contract() {
        assert_eq!(
            agent_pty_args("http://node", "codex", "team-codex", ""),
            [
                "agent",
                "pty",
                "codex",
                "--node",
                "http://node",
                "--cred",
                "team-codex",
            ]
        );
        let sched = agent_sched_args("http://node", "claude", "team-claude", "", "hello");
        assert_eq!(sched.last().map(String::as_str), Some("hello"));
        assert_eq!(
            &sched[..8],
            [
                "agent",
                "sched",
                "claude",
                "--cred",
                "team-claude",
                "--node",
                "http://node",
                "--"
            ]
        );
    }

    /// `--node` and `--host-node` are different questions: the first is who the
    /// CLI dials, the second is who executes. An unpicked host must emit NO
    /// flag — the argv above, byte for byte — or every default run silently
    /// changes shape.
    #[test]
    fn a_picked_host_node_is_the_only_thing_that_adds_the_flag() {
        let peer = "b".repeat(64);
        assert_eq!(
            agent_pty_args("http://node", "codex", "team-codex", &peer),
            [
                "agent",
                "pty",
                "codex",
                "--node",
                "http://node",
                "--cred",
                "team-codex",
                "--host-node",
                peer.as_str(),
            ]
        );
        assert_eq!(
            agent_sched_args("http://node", "claude", "team-claude", &peer, "hello"),
            [
                "agent",
                "sched",
                "claude",
                "--cred",
                "team-claude",
                "--node",
                "http://node",
                "--host-node",
                peer.as_str(),
                "--",
                "hello",
            ]
        );
        for blank in ["", "   "] {
            assert!(
                !agent_pty_args("http://node", "codex", "team-codex", blank)
                    .contains(&"--host-node".to_string())
            );
            assert!(
                !agent_sched_args("http://node", "claude", "team-claude", blank, "hello")
                    .contains(&"--host-node".to_string())
            );
        }
    }

    /// The picker's rows and the reverse lookup are one spelling. The local row
    /// — and any label the registry dropped — must resolve to no host key at
    /// all, which is what keeps the flag off the default argv.
    #[test]
    fn the_host_picker_round_trips_a_row_to_its_node_key() {
        let rows = vec![AgentHostNode {
            key: "a".repeat(64),
            label: "alice".into(),
            providers: vec!["codex".into(), "claude".into()],
        }];
        assert_eq!(
            agent_host_node_options(rows.clone()),
            ["This node", "alice · codex, claude"]
        );
        assert_eq!(
            agent_host_node_key(rows.clone(), "alice · codex, claude".into()),
            "a".repeat(64)
        );
        assert_eq!(agent_host_node_key(rows.clone(), LOCAL_HOST_NODE.into()), "");
        assert_eq!(agent_host_node_key(rows, "a node that left".into()), "");
    }

    /// A node that announces nothing this app can launch is not a compute
    /// choice, and an unnamed account still has to read as something.
    #[test]
    fn only_launchable_announcements_become_host_rows() {
        let key = vec![0xab, 0xcd, 0xef, 0x01, 0x23];
        let names = HashMap::from([(key.clone(), "alice".to_string())]);
        assert!(host_node_row(&names, &key, vec!["storage".into()]).is_none());
        assert!(host_node_row(&names, &key, Vec::new()).is_none());
        let named = host_node_row(&names, &key, vec!["codex".into(), "storage".into()]).unwrap();
        assert_eq!((named.label.as_str(), named.providers.len()), ("alice", 1));
        assert_eq!(named.key, "abcdef0123");
        let unnamed = host_node_row(&HashMap::new(), &key, vec!["claude".into()]).unwrap();
        assert_eq!(unnamed.label, "abcdef01…");
    }

    /// The local row is one string in two languages: Rust rebuilds the option
    /// list on every registry read, Ice seeds both the list and the selection
    /// for the frames before the first one lands. Drift and the picker opens on
    /// a label its own list does not carry.
    #[test]
    fn the_local_host_row_matches_the_shell_state_default() {
        let state = include_str!("../ui/state/shell.ice");
        for seed in [
            format!("shell_host_node_options:[str] = [\"{LOCAL_HOST_NODE}\"]"),
            format!("shell_host_node = \"{LOCAL_HOST_NODE}\""),
        ] {
            assert!(
                state.contains(&seed),
                "the shell state must seed the local host row: {seed}"
            );
        }
    }

    #[test]
    fn credentials_follow_the_selected_provider() {
        let rows = vec![
            AgentCredential {
                name: "c1".into(),
                provider: "claude".into(),
            },
            AgentCredential {
                name: "x1".into(),
                provider: "codex".into(),
            },
        ];
        assert_eq!(agent_credential_names(rows.clone(), "codex".into()), ["x1"]);
        assert_eq!(
            agent_credential_choice(rows, "codex".into(), "c1".into()),
            "x1"
        );
    }

    #[test]
    fn conversation_prompt_keeps_roles_and_the_latest_turn() {
        let entries = vec![
            AgentChatEntry {
                id: 1,
                role: "user".into(),
                body: "first".into(),
                provider: "codex".into(),
            },
            AgentChatEntry {
                id: 2,
                role: "assistant".into(),
                body: "second".into(),
                provider: "codex".into(),
            },
            AgentChatEntry {
                id: 3,
                role: "user".into(),
                body: "third".into(),
                provider: "codex".into(),
            },
        ];
        let prompt = agent_chat_prompt(entries);
        assert!(prompt.contains("User: first"));
        assert!(prompt.contains("Assistant: second"));
        assert!(prompt.ends_with("User: third"));
    }

    #[test]
    fn run_id_exposes_only_a_wire_valid_dispatch_id() {
        let dispatch = "a".repeat(64);
        assert_eq!(
            dispatch_id_from_saga(&format!("origin/sched\u{1f}{dispatch}")).unwrap(),
            dispatch
        );
        assert!(dispatch_id_from_saga("origin/no-dispatch").is_err());
        assert!(dispatch_id_from_saga("origin/sched\u{1f}abc").is_err());
    }

    #[test]
    fn provider_output_projects_known_events_not_raw_json() {
        let line = serde_json::json!({
            "type": "item.completed",
            "item": { "type": "agent_message", "text": "answer" }
        })
        .to_string();
        let event = provider_output_event("codex", &line, 7).unwrap();
        assert_eq!(event.kind, "preview");
        assert_eq!(event.answer, "answer");
        assert!(provider_output_event("codex", r#"{"type":"unknown","secret":"no"}"#, 8).is_none());
    }

    #[test]
    fn durable_result_projects_the_answer_not_the_runner_receipt() {
        let wrapped = serde_json::json!({
            "ducktape_runner_result": 1,
            "response_text": "UI LIVE OK",
            "workspace_receipt": {
                "source_prefix": "/shared/agent-workspaces/sched",
                "source_snapshot": null,
                "output_snapshot": null,
                "commit_height": null,
                "rebased": false,
                "no_changes": true
            }
        });
        assert_eq!(
            agent_response_text(wrapped.to_string().as_bytes()).unwrap(),
            "UI LIVE OK"
        );

        let mut drifted = wrapped;
        drifted["ducktape_runner_result"] = serde_json::json!(2);
        assert!(agent_response_text(drifted.to_string().as_bytes()).is_err());
        drifted["ducktape_runner_result"] = serde_json::json!(1);
        drifted["legacy"] = serde_json::json!(true);
        assert!(agent_response_text(drifted.to_string().as_bytes()).is_err());
    }
}
