//! `ducktape agent` — remote/interactive sandboxed provider sessions.
//!
//! Two verbs, one credential+targeting story:
//!
//! - `agent pty [<provider>] [--node <name>] [--cred <name>] [--cpu <n>] [--mem <gb>]`
//!   attaches THIS terminal to a Podman-sandboxed provider running on a host
//!   node (default: this node). The CLI talks ONLY to its own node's ws surface
//!   (`/v1/ws`); the node does the cross-node mesh. Raw terminal mode + resize
//!   forwarding make it feel like ssh.
//! - `agent sched [<provider>] --cred <name> [--node <name>] [--cpu] [--mem] -- "<prompt>"`
//!   submits a durable, node-pinned headless run (a `saga::SagaMsg::Trigger`)
//!   and prints its run id. The target may be offline now and execute on
//!   reconnect — that durability is the point.
//!
//! `<provider>` is optional when `--cred` names a credential: the registry
//! record's kind decides what to launch; an explicit provider contradicting the
//! cred is an error. `--node` resolves a display name → account → node key (or
//! accepts a raw 64-hex node key), erroring with candidates when an account
//! operates several nodes.
//!
//! Program output stays `println!` (a CLI's stdout is not logging); the pty
//! passthrough writes raw provider bytes straight to stdout.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::config::{self, hex_bytes};
use crate::cred_cli::{ProviderArg, query_node};

type AgentResult = Result<(), Box<dyn std::error::Error>>;

/// `ducktape agent <verb>`. `-n/--network` selects THIS operator's own node
/// (the ws + query surface the CLI dials); it is global so it attaches in any
/// position. The own node is also found from `DUCKTAPE_NODE` or the lone
/// registered workspace.
#[derive(Debug, clap::Args)]
pub(crate) struct AgentArgs {
    #[command(subcommand)]
    cmd: AgentCmd,
    /// this operator's own node: a registered workspace chain id (else
    /// `DUCKTAPE_NODE`, else the single registered workspace)
    #[arg(short = 'n', long = "network", value_name = "CHAIN-ID", global = true)]
    network: Option<String>,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum AgentCmd {
    /// attach this terminal to a sandboxed provider (raw pty, resize-aware)
    Pty(PtyArgs),
    /// submit a durable headless run pinned to a node; prints its run id
    Sched(SchedArgs),
}

#[derive(Debug, clap::Args)]
pub(crate) struct PtyArgs {
    /// provider to launch (`claude`|`codex`); optional when `--cred` names one
    provider: Option<ProviderArg>,
    /// host node to run on: a display name or a raw 64-hex node key
    /// (omitted = this node)
    #[arg(long, value_name = "NAME")]
    node: Option<String>,
    /// credential name to serve the session (required for a cross-node host)
    #[arg(long, value_name = "NAME")]
    cred: Option<String>,
    /// cpu-cores ceiling for the sandbox
    #[arg(long, value_name = "CORES")]
    cpu: Option<u64>,
    /// memory ceiling in GB for the sandbox
    #[arg(long, value_name = "GB")]
    mem: Option<u64>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SchedArgs {
    /// provider to launch (`claude`|`codex`); optional — the `--cred` kind decides
    provider: Option<ProviderArg>,
    /// credential name (required: a headless guest run must bring a credential)
    #[arg(long, value_name = "NAME")]
    cred: String,
    /// node to pin the run to: a display name or a raw 64-hex node key
    /// (omitted = this node)
    #[arg(long, value_name = "NAME")]
    node: Option<String>,
    /// cpu-cores demand
    #[arg(long, value_name = "CORES")]
    cpu: Option<u64>,
    /// memory demand in GB
    #[arg(long, value_name = "GB")]
    mem: Option<u64>,
    /// the prompt, after `--`
    #[arg(last = true, value_name = "PROMPT", required = true)]
    prompt: String,
}

pub(crate) fn run(args: AgentArgs) -> AgentResult {
    let AgentArgs { cmd, network } = args;
    match cmd {
        AgentCmd::Pty(pty) => cmd_pty(pty, network.as_deref()),
        AgentCmd::Sched(sched) => cmd_sched(sched, network.as_deref()),
    }
}

// ============================================================================
// pty — create the session, then attach this terminal in raw mode
// ============================================================================

fn cmd_pty(args: PtyArgs, network: Option<&str>) -> AgentResult {
    let base = own_node_base(network)?;
    let provider = resolve_provider(&base, args.provider, args.cred.as_deref())?;
    let host_hex = match args.node.as_deref() {
        Some(name) => Some(hex_bytes(&resolve_host_node(&base, name)?)),
        None => None,
    };

    let created = create_session(
        &base,
        provider.token(),
        host_hex.as_deref(),
        args.cred.as_deref(),
        args.cpu,
        args.mem,
    )?;
    eprintln!("attached to {} ({})", created.session_id, created.topic);

    // A dedicated single-thread runtime drives the ws pump; the raw-mode guard
    // lives on the pump's own stack so it restores the tty on normal exit AND on
    // a panic unwinding through it.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("attach runtime: {e}"))?;
    let outcome = runtime.block_on(attach(&base, &created.session_id, &created.topic));
    drop(runtime);

    // Best-effort close (idempotent host-side; the 4 h wall-clock + kill-on-drop
    // are the backstops if it never lands).
    let _ = close_session(&base, &created.session_id);
    outcome
}

/// the create reply — `{ session_id, topic }`.
struct Created {
    session_id: String,
    topic: String,
}

/// `POST /v1/term/sessions` on the operator's own node. The node routes locally
/// or over the mesh to the host; its refusal strings (`host refused: …`,
/// `a cross-node session requires --cred`, …) come back verbatim.
fn create_session(
    base: &str,
    provider: &str,
    node_hex: Option<&str>,
    cred: Option<&str>,
    cpu: Option<u64>,
    mem_gb: Option<u64>,
) -> Result<Created, Box<dyn std::error::Error>> {
    let mut body = serde_json::json!({ "agent": provider, "mode": "single" });
    if let Some(node) = node_hex {
        body["node"] = serde_json::Value::String(node.to_string());
    }
    if let Some(cred) = cred {
        body["cred"] = serde_json::Value::String(cred.to_string());
    }
    if let Some(cpu) = cpu {
        body["cpu"] = serde_json::Value::Number(cpu.into());
    }
    if let Some(mem_gb) = mem_gb {
        body["mem_gb"] = serde_json::Value::Number(mem_gb.into());
    }

    let resp = reqwest::blocking::Client::new()
        .post(format!("{base}/v1/term/sessions"))
        .json(&body)
        .send()
        .map_err(|e| format!("POST {base}/v1/term/sessions: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(error_field(&text).into());
    }
    let value: serde_json::Value = serde_json::from_str(&text)?;
    let session_id = value["session_id"]
        .as_str()
        .ok_or_else(|| format!("create reply missing session_id: {text}"))?
        .to_string();
    let topic = value["topic"]
        .as_str()
        .ok_or_else(|| format!("create reply missing topic: {text}"))?
        .to_string();
    Ok(Created { session_id, topic })
}

fn close_session(base: &str, session_id: &str) -> Result<(), Box<dyn std::error::Error>> {
    reqwest::blocking::Client::new()
        .post(format!("{base}/v1/term/sessions/{session_id}/close"))
        .send()
        .map_err(|e| format!("close session: {e}"))?;
    Ok(())
}

/// Attach this terminal to the session's ws output topic and forward keystrokes
/// and resizes. Raw mode is entered on this stack so its guard restores the tty
/// whichever way this future ends.
async fn attach(base: &str, session_id: &str, topic: &str) -> AgentResult {
    use futures::{SinkExt as _, StreamExt as _};
    use tokio::io::{AsyncReadExt as _, AsyncWriteExt as _};
    use tokio::signal::unix::{SignalKind, signal};
    use tokio_tungstenite::tungstenite::Message;

    let (socket, _resp) = tokio_tungstenite::connect_async(ws_url(base))
        .await
        .map_err(|e| format!("connect ws /v1/ws: {e}"))?;
    let (mut ws_tx, mut ws_rx) = socket.split();

    // raw mode for the whole attach; the guard restores on drop (normal + panic).
    let _raw = crate::tty::RawGuard::enter();
    let stdin_fd = libc::STDIN_FILENO;

    // one outbound lane: subscribe (the entitlement gate) must reach the node
    // before any input, so every client frame funnels through this ordered mpsc.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(256);
    out_tx.send(subscribe_frame(topic)).await.ok();
    let (cols, rows) = window_size(stdin_fd);
    out_tx.send(resize_frame(session_id, cols, rows)).await.ok();

    let writer = tokio::spawn(async move {
        while let Some(text) = out_rx.recv().await {
            if ws_tx.send(Message::text(text)).await.is_err() {
                break;
            }
        }
        let _ = ws_tx.close().await;
    });

    let input_tx = out_tx.clone();
    let input_session = session_id.to_string();
    let stdin_task = tokio::spawn(async move {
        let mut stdin = tokio::io::stdin();
        let mut buf = [0u8; 4096];
        loop {
            let read = match stdin.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let data = STANDARD.encode(&buf[..read]);
            if input_tx
                .send(input_frame(&input_session, &data))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    let mut winch = signal(SignalKind::window_change()).map_err(|e| format!("SIGWINCH: {e}"))?;
    let mut term = signal(SignalKind::terminate()).map_err(|e| format!("SIGTERM: {e}"))?;
    let mut hup = signal(SignalKind::hangup()).map_err(|e| format!("SIGHUP: {e}"))?;
    let mut stdout = tokio::io::stdout();

    loop {
        tokio::select! {
            frame = ws_rx.next() => {
                let Some(Ok(message)) = frame else { break };
                if message.is_close() {
                    break;
                }
                if let Message::Text(text) = message {
                    // the session's child exited — the node signals the topic is
                    // over. Stop attaching (the wedge fix): without this the loop
                    // blocks on a dead topic and no keystroke, not even Ctrl-C,
                    // can end it (input is dropped as the session is gone).
                    if is_term_ended(&text) {
                        break;
                    }
                    if let Some(bytes) = decode_term_chunk(&text) {
                        stdout.write_all(&bytes).await.map_err(|e| format!("stdout: {e}"))?;
                        stdout.flush().await.map_err(|e| format!("stdout flush: {e}"))?;
                    }
                }
            }
            _ = winch.recv() => {
                let (cols, rows) = window_size(stdin_fd);
                out_tx.send(resize_frame(session_id, cols, rows)).await.ok();
            }
            _ = term.recv() => break,
            _ = hup.recv() => break,
        }
    }
    stdin_task.abort();
    writer.abort();
    Ok(())
}


/// the tty window size (cols, rows), or an 80x24 fallback when the ioctl fails.
fn window_size(fd: i32) -> (u16, u16) {
    // SAFETY: `ws` is fully written by a successful ioctl; on failure we ignore it.
    unsafe {
        let mut ws: libc::winsize = std::mem::zeroed();
        let ok = libc::ioctl(fd, libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col != 0;
        if ok { (ws.ws_col, ws.ws_row) } else { (80, 24) }
    }
}

// ============================================================================
// sched — a node-pinned durable saga trigger
// ============================================================================

fn cmd_sched(args: SchedArgs, network: Option<&str>) -> AgentResult {
    let base = own_node_base(network)?;
    let provider = resolve_provider(&base, args.provider, Some(&args.cred))?;
    let tag = provider.token();

    let target = match args.node.as_deref() {
        Some(name) => resolve_host_node(&base, name)?.to_vec(),
        None => own_node_key(&base)?,
    };
    preflight_provider(&base, &target, tag)?;

    let dispatch_id = fresh_dispatch_id();
    let saga_id = format!("sched\u{1f}{dispatch_id}");
    let payload =
        dispatch_oracle::envelope::compose_headless(&saga_id, &args.prompt, Some(&args.cred))
            .into_bytes();

    let mut demands = BTreeMap::new();
    if let Some(cpu) = args.cpu {
        demands.insert("cores".to_string(), cpu);
    }
    if let Some(mem) = args.mem {
        demands.insert("mem_gb".to_string(), mem);
    }

    let spec = dispatch::WorkSpec {
        kind: dispatch::WORK_SPEC_KIND.to_string(),
        dispatch_id: dispatch_id.clone(),
        capability: tag.to_string(),
        payload,
        demands: demands.clone(),
        admission: dispatch::AdmissionPolicy::Queue,
    };
    let trigger = saga::SagaMsg::Trigger {
        saga_id: saga_id.clone(),
        spec: dispatch::encode_work_spec(&spec),
        reply_to: None,
        reply_payload: Vec::new(),
        deadline: None,
        max_attempts: 3,
        lease_views: None,
        capability: Some(tag.to_string()),
        demands,
        pinned_assignee: Some(target),
    };

    submit(&base, "saga", serde_json::to_value(&trigger)?)?;
    println!("{saga_id}");
    Ok(())
}

/// Fail early when the registry KNOWS the target advertises no matching
/// provider. An empty announcement (offline/never-announced node) is NOT a
/// failure — a dark pinned node executes on reconnect; that durability is the
/// contract, so we let the saga carry it.
fn preflight_provider(base: &str, target: &[u8], tag: &str) -> AgentResult {
    let query = capability::CapabilityQuery::Node {
        node: target.to_vec(),
    };
    let value = query_node(base, "capability", serde_json::to_value(&query)?)?;
    let announced = match serde_json::from_value::<capability::CapabilityReply>(value)? {
        capability::CapabilityReply::Node(tags) => tags,
        other => return Err(format!("unexpected capability reply: {other:?}").into()),
    };
    let advertises_something = !announced.is_empty();
    let missing_tag = !announced.iter().any(|t| t == tag);
    if advertises_something && missing_tag {
        return Err(format!(
            "the target node advertises no {tag} provider (announces: {})",
            announced.join(", ")
        )
        .into());
    }
    Ok(())
}

/// One `POST /v1/submit` `{target, payload}` — the module reply/receipt on
/// success, the node's rejection string on failure.
fn submit(base: &str, target: &str, payload: serde_json::Value) -> AgentResult {
    let resp = reqwest::blocking::Client::new()
        .post(format!("{base}/v1/submit"))
        .json(&serde_json::json!({ "target": target, "payload": payload }))
        .send()
        .map_err(|e| format!("POST {base}/v1/submit: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    if !status.is_success() {
        return Err(error_field(&text).into());
    }
    Ok(())
}

/// A fresh 16-byte run nonce, hex — the dispatch id `run-output:<id>` keys on.
fn fresh_dispatch_id() -> String {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    hex_bytes(&bytes)
}

// ============================================================================
// shared resolution
// ============================================================================

/// The operator's own-node http base: an explicit `-n/--network` wins, then
/// `DUCKTAPE_NODE`, then the single registered workspace.
fn own_node_base(network: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(needle) = network.filter(|n| !n.is_empty()) {
        return http_of_workspace(needle);
    }
    if let Ok(url) = std::env::var("DUCKTAPE_NODE")
        && !url.is_empty()
    {
        return Ok(url.trim_end_matches('/').to_string());
    }
    let mut workspaces = config::list_workspaces()?;
    match workspaces.len() {
        1 => {
            let (chain_id, _path) = workspaces.swap_remove(0);
            http_of_workspace(&chain_id)
        }
        0 => Err("no workspace selected: pass -n/--network <id> or set DUCKTAPE_NODE".into()),
        _ => {
            let list = workspaces
                .iter()
                .map(|(chain_id, _)| format!("  {chain_id}"))
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!("several workspaces registered — pick one with -n:\n{list}").into())
        }
    }
}

fn http_of_workspace(needle: &str) -> Result<String, Box<dyn std::error::Error>> {
    let (_dir, http) = config::resolve_network(needle)?;
    http.map(|base| base.trim_end_matches('/').to_string())
        .ok_or_else(|| {
            format!("workspace {needle:?} has no http listen — its node.toml sets no http_listen")
                .into()
        })
}

/// This node's own 32-byte mesh key, from `/v1/status`'s `public_key`.
fn own_node_key(base: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let resp = reqwest::blocking::Client::new()
        .get(format!("{base}/v1/status"))
        .send()
        .map_err(|e| format!("GET {base}/v1/status: {e}"))?;
    let value: serde_json::Value = resp.json().map_err(|e| format!("status reply: {e}"))?;
    let hex = value["public_key"]
        .as_str()
        .filter(|hex| !hex.is_empty())
        .ok_or("this node has no mesh identity — pass --node to pick a host")?;
    config::unhex(hex).map_err(|e| format!("node key hex: {e}").into())
}

/// Resolve the pty/sched provider: `--cred`'s registered kind decides, and an
/// explicit provider contradicting it is an error. Without `--cred`, a provider
/// is required.
fn resolve_provider(
    base: &str,
    provider: Option<ProviderArg>,
    cred: Option<&str>,
) -> Result<ProviderArg, Box<dyn std::error::Error>> {
    let Some(name) = cred else {
        return provider
            .ok_or_else(|| "a provider (claude|codex) is required without --cred".into());
    };
    let record =
        query_credential(base, name)?.ok_or_else(|| format!("unknown credential {name:?}"))?;
    let from_cred = provider_from_kind(record.kind);
    if let Some(explicit) = provider
        && explicit != from_cred
    {
        return Err(format!(
            "provider {} contradicts credential {name:?} (kind {})",
            explicit.token(),
            from_cred.token()
        )
        .into());
    }
    Ok(from_cred)
}

fn provider_from_kind(kind: gateway::CredentialKind) -> ProviderArg {
    match kind {
        gateway::CredentialKind::Claude => ProviderArg::Claude,
        gateway::CredentialKind::Codex => ProviderArg::Codex,
    }
}

fn query_credential(
    base: &str,
    name: &str,
) -> Result<Option<gateway::CredentialRecord>, Box<dyn std::error::Error>> {
    let query = gateway::GatewayQuery::Credential {
        name: name.to_string(),
    };
    let value = query_node(base, "gateway", serde_json::to_value(&query)?)?;
    match serde_json::from_value::<gateway::GatewayReply>(value)? {
        gateway::GatewayReply::Credential(record) => Ok(record),
        other => Err(format!("unexpected gateway reply: {other:?}").into()),
    }
}

/// Resolve a `--node` target to a 32-byte node key: a raw 64-hex key is used
/// directly; a display name resolves through identity to its account's node,
/// erroring with candidates when the account operates several.
fn resolve_host_node(base: &str, name: &str) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    if let Some(key) = decode_node_key(name) {
        return Ok(key);
    }
    let query = identity::IdentityQuery::All {
        from: 0,
        limit: u64::MAX,
    };
    let value = query_node(base, "identity", serde_json::to_value(&query)?)?;
    let accounts = match serde_json::from_value::<identity::IdentityReply>(value)? {
        identity::IdentityReply::Accounts(accounts) => accounts,
        other => return Err(format!("unexpected identity reply: {other:?}").into()),
    };
    let matches: Vec<&identity::AccountView> = accounts
        .iter()
        .filter(|account| account.display_name.as_deref() == Some(name))
        .collect();
    let account = match matches.as_slice() {
        [only] => only,
        [] => {
            return Err(format!("no account named {name:?} (nor a valid 64-hex node key)").into());
        }
        many => {
            return Err(format!(
                "account name {name:?} is ambiguous across {} accounts",
                many.len()
            )
            .into());
        }
    };
    match account.nodes.as_slice() {
        [only] => decode_node_key_bytes(&only.node_key),
        [] => Err(format!("account {name:?} has no bound node").into()),
        many => {
            let candidates = many
                .iter()
                .map(|node| format!("  {}", hex_bytes(&node.node_key)))
                .collect::<Vec<_>>()
                .join("\n");
            Err(format!(
                "account {name:?} operates {} nodes — pass one by hex node key:\n{candidates}",
                many.len()
            )
            .into())
        }
    }
}

/// Decode a 64-hex string to 32 bytes, or `None` when it is not one.
fn decode_node_key(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let bytes = config::unhex(text).ok()?;
    <[u8; 32]>::try_from(bytes).ok()
}

fn decode_node_key_bytes(bytes: &[u8]) -> Result<[u8; 32], Box<dyn std::error::Error>> {
    <[u8; 32]>::try_from(bytes)
        .map_err(|_| format!("bound node key is not 32 bytes ({} bytes)", bytes.len()).into())
}

/// `http(s)://host:port` → `ws(s)://host:port/v1/ws`, the node's own ws surface.
fn ws_url(base: &str) -> String {
    let ws_base = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    };
    format!("{}/v1/ws", ws_base.trim_end_matches('/'))
}

/// Pull the `"error"` field out of a node error body, or fall back to the raw
/// text — so the node's verbatim refusal strings reach the operator.
fn error_field(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v["error"].as_str().map(str::to_string))
        .unwrap_or_else(|| body.to_string())
}

fn subscribe_frame(topic: &str) -> String {
    serde_json::json!({ "op": "subscribe", "topics": [topic] }).to_string()
}

fn input_frame(session: &str, data_b64: &str) -> String {
    serde_json::json!({ "op": "term_input", "session": session, "data": data_b64 }).to_string()
}

fn resize_frame(session: &str, cols: u16, rows: u16) -> String {
    serde_json::json!({ "op": "term_resize", "session": session, "cols": cols, "rows": rows })
        .to_string()
}

/// Decode one server ws text frame to the raw pty bytes it carries, or `None`
/// for any non-output frame (subscribed, heartbeat, module event, error).
fn decode_term_chunk(text: &str) -> Option<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    if value["type"].as_str() != Some("event") {
        return None;
    }
    let item = value["item"].as_str()?;
    STANDARD.decode(item).ok()
}

/// the node's terminal frame for this topic: the session's child exited and the
/// `term:<id>` topic is complete (`{"type":"term_ended",...}`). The signal the
/// attach loop ends on — see [`stream::ServerFrame::TermEnded`].
fn is_term_ended(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .as_ref()
        .and_then(|v| v["type"].as_str())
        == Some("term_ended")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cred_kind_wins_and_contradiction_is_an_error() {
        // The reverse map is the whole authority when a provider is omitted.
        assert_eq!(
            provider_from_kind(gateway::CredentialKind::Claude),
            ProviderArg::Claude
        );
        assert_eq!(
            provider_from_kind(gateway::CredentialKind::Codex),
            ProviderArg::Codex
        );
    }

    #[test]
    fn node_key_hex_round_trips_and_rejects_bad_len() {
        let hex = "ab".repeat(32); // 64 hex chars = 32 bytes
        assert_eq!(decode_node_key(&hex), Some([0xab; 32]));
        assert_eq!(decode_node_key("abcd"), None); // too short
        assert_eq!(decode_node_key(&"zz".repeat(32)), None); // not hex
    }

    #[test]
    fn ws_url_maps_scheme_and_appends_path() {
        assert_eq!(ws_url("http://127.0.0.1:8080"), "ws://127.0.0.1:8080/v1/ws");
        assert_eq!(ws_url("https://host:9/"), "wss://host:9/v1/ws");
    }

    #[test]
    fn term_chunk_decodes_only_output_frames() {
        let chunk = serde_json::json!({
            "type": "event", "topic": "term:abc", "cursor": "3", "item": STANDARD.encode(b"hi")
        })
        .to_string();
        assert_eq!(decode_term_chunk(&chunk), Some(b"hi".to_vec()));

        let module_event = serde_json::json!({
            "type": "event", "topic": "chat", "cursor": "1", "op": { "height": 1 }
        })
        .to_string();
        assert_eq!(decode_term_chunk(&module_event), None);

        let heartbeat = serde_json::json!({ "type": "heartbeat", "height": 1 }).to_string();
        assert_eq!(decode_term_chunk(&heartbeat), None);
    }

    #[test]
    fn term_ended_frame_ends_the_attach_but_output_chunks_do_not() {
        // the node's terminal signal (ServerFrame::TermEnded) — the attach loop
        // breaks on it; an output chunk or any other frame keeps it running.
        let ended = serde_json::json!({ "type": "term_ended", "topic": "term:abc" }).to_string();
        assert!(is_term_ended(&ended));

        let chunk = serde_json::json!({
            "type": "event", "topic": "term:abc", "cursor": "3", "item": STANDARD.encode(b"hi")
        })
        .to_string();
        assert!(!is_term_ended(&chunk));
        assert!(!is_term_ended(&serde_json::json!({ "type": "heartbeat" }).to_string()));
    }

    #[test]
    fn client_frames_carry_the_snake_case_op_tags() {
        let sub: serde_json::Value = serde_json::from_str(&subscribe_frame("term:x")).unwrap();
        assert_eq!(sub["op"], "subscribe");
        assert_eq!(sub["topics"][0], "term:x");

        let input: serde_json::Value =
            serde_json::from_str(&input_frame("sid", "ZGF0YQ==")).unwrap();
        assert_eq!(input["op"], "term_input");
        assert_eq!(input["session"], "sid");
        assert_eq!(input["data"], "ZGF0YQ==");

        let resize: serde_json::Value =
            serde_json::from_str(&resize_frame("sid", 120, 40)).unwrap();
        assert_eq!(resize["op"], "term_resize");
        assert_eq!(resize["cols"], 120);
        assert_eq!(resize["rows"], 40);
    }

    #[test]
    fn error_field_prefers_the_error_key() {
        assert_eq!(
            error_field(r#"{"error":"host refused: no_sandbox"}"#),
            "host refused: no_sandbox"
        );
        assert_eq!(error_field("raw text"), "raw text");
    }
}
