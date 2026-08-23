//! `ducktape agent` — remote/interactive sandboxed provider sessions.
//!
//! Two verbs, one credential+targeting story:
//!
//! - `agent pty [<provider>] [--host-node <name>] [--cred <name>] [--cpu <n>] [--mem <gb>]`
//!   attaches THIS terminal to a provider running in a microVM on a host
//!   node (default: this node). The CLI talks ONLY to its own node's ws surface
//!   (`/v1/ws`); the node does the cross-node mesh. Raw terminal mode + resize
//!   forwarding make it feel like ssh.
//! - `agent sched [<provider>] --cred <name> [--host-node <name>] [--cpu] [--mem] -- "<prompt>"`
//!   submits a durable, node-pinned headless run (a `saga::SagaMsg::Trigger`)
//!   and prints its run id. The target may be offline now and execute on
//!   reconnect — that durability is the point.
//!
//! `<provider>` is optional when `--cred` names a credential: the registry
//! record's kind decides what to launch; an explicit provider contradicting the
//! cred is an error.
//!
//! TWO addressing inputs, deliberately two names. `--node`/`-n`/`DUCKTAPE_NODE`
//! (the shared [`NodeAddr`] group) say which node this CLI DIALS — an http base.
//! `--host-node` says which PEER runs the work: a display name → account → node
//! key, or a raw 64-hex node key, erroring with candidates when an account
//! operates several nodes. They are different types; spelling both `--node`
//! is what made the flag mean two things.
//!
//! Program output stays `println!` (a CLI's stdout is not logging); the pty
//! passthrough writes raw provider bytes straight to stdout.

use std::collections::BTreeMap;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;

use crate::cli_args::NodeAddr;
use crate::config::{self, hex_bytes};
use crate::cred_cli::{ProviderArg, query_node};

type AgentResult = Result<(), Box<dyn std::error::Error>>;

/// `ducktape agent <verb>`. The shared addressing group selects THIS operator's
/// own node — the ws + query surface the CLI dials, never the host the work runs
/// on (that is `--host-node`).
#[derive(Debug, clap::Args)]
pub(crate) struct AgentArgs {
    #[command(subcommand)]
    cmd: AgentCmd,
    #[command(flatten)]
    addr: NodeAddr,
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
    /// host node to RUN on: a display name or a raw 64-hex node key
    /// (omitted = this node). NOT `--node`, which is the http base this CLI dials.
    #[arg(long = "host-node", value_name = "NAME")]
    host_node: Option<String>,
    /// credential name to serve the session (required for a cross-node host)
    #[arg(long, value_name = "NAME")]
    cred: Option<String>,
    /// cpu-cores ceiling for the sandbox (minimum 2)
    #[arg(long, value_name = "CORES", value_parser = at_least_the_sandbox_floor)]
    cpu: Option<u64>,
    /// memory ceiling in GB for the sandbox
    #[arg(long, value_name = "GB")]
    mem: Option<u64>,
}

#[derive(Debug, clap::Args)]
pub(crate) struct SchedArgs {
    /// provider to launch (`claude`|`codex`); optional — the `--cred` kind decides
    provider: Option<ProviderArg>,
    /// credential name (required: a headless guest run must bring a credential).
    /// With `--host-node`, THIS RUN LETS THAT NODE SPEND YOUR SUBSCRIPTION: the
    /// lender admits the executing node on YOUR grant, for this credential and
    /// this run only, until the run reaches a terminal status.
    #[arg(long, value_name = "NAME")]
    cred: String,
    /// node to PIN the run to: a display name or a raw 64-hex node key
    /// (omitted = this node). NOT `--node`, which is the http base this CLI dials.
    /// The pin is what scopes the `--cred` draw — it is the only node that may
    /// present this run as its reason for opening a session.
    #[arg(long = "host-node", value_name = "NAME")]
    host_node: Option<String>,
    /// cpu-cores demand (minimum 2)
    #[arg(long, value_name = "CORES", value_parser = at_least_the_sandbox_floor)]
    cpu: Option<u64>,
    /// memory demand in GB
    #[arg(long, value_name = "GB")]
    mem: Option<u64>,
    /// the prompt, after `--`
    #[arg(last = true, value_name = "PROMPT", required = true)]
    prompt: String,
}

/// Refuse a core count no sandbox will accept, AT SUBMIT.
///
/// A zero-core run is accepted by placement and by the lease, and refused only
/// by the spawn on the executing box — the most expensive possible place to
/// find out, because it burns every `RUN_MAX_ATTEMPTS` retry and fails the saga
/// having told the submitter nothing they could act on.
///
/// The floor is 1: a VM is BUILT at a size, so zero vCPUs is not a smaller
/// machine — it is not a machine.
fn at_least_the_sandbox_floor(value: &str) -> Result<u64, String> {
    let cores: u64 = value
        .parse()
        .map_err(|_| format!("{value:?} is not a number of cores"))?;
    if cores == 0 {
        return Err("a sandboxed run needs at least 1 core — try --cpu 1".to_string());
    }
    Ok(cores)
}

pub(crate) fn run(args: AgentArgs) -> AgentResult {
    let AgentArgs { cmd, addr } = args;
    let base = addr.resolve()?;
    match cmd {
        // pty takes the whole group, not just the resolved base: attaching needs
        // the node's WORKSPACE too (its 0600 service-link token admits the
        // session's ws topic), and only the ladder knows which workspace the
        // address it just resolved belongs to.
        AgentCmd::Pty(pty) => cmd_pty(pty, &base, &addr),
        AgentCmd::Sched(sched) => cmd_sched(sched, &base),
    }
}

// ============================================================================
// pty — create the session, then attach this terminal in raw mode
// ============================================================================

fn cmd_pty(args: PtyArgs, base: &str, addr: &NodeAddr) -> AgentResult {
    // read BEFORE the create: the ws surface admits a session's topic against
    // this secret, so a workspace we cannot read is a session we could never
    // attach to — and failing here costs no container.
    let secret = workspace_secret(addr)?;
    let provider = resolve_provider(base, args.provider, args.cred.as_deref())?;
    let host_hex = match args.host_node.as_deref() {
        Some(name) => Some(hex_bytes(&resolve_host_node(base, name)?)),
        None => None,
    };

    let created = create_session(
        base,
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
    let outcome =
        runtime.block_on(attach(base, &created.session_id, &created.topic, &secret));
    // `shutdown_background`, NOT drop: the attach loop's stdin forwarder reads
    // `tokio::io::stdin()`, which parks a BLOCKING thread on `read(0)`. On a real
    // tty that read never returns, and `abort()` cannot interrupt an OS-level
    // blocking read — so a normal runtime drop WAITS for that thread forever,
    // wedging `agent pty` AFTER the session already ended (the second half of the
    // wedge; the first was the missing end signal). Detach instead: the stuck
    // reader dies with the process.
    runtime.shutdown_background();

    // Best-effort close (idempotent host-side; the 4 h wall-clock + kill-on-drop
    // are the backstops if it never lands).
    let _ = close_session(base, &created.session_id);
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
async fn attach(base: &str, session_id: &str, topic: &str, secret: &str) -> AgentResult {
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

    // one outbound lane: the subscribe carries the node's workspace secret and
    // is what ADMITS this connection to the session, so it must reach the node
    // before any input — every client frame funnels through this ordered mpsc.
    let (out_tx, mut out_rx) = tokio::sync::mpsc::channel::<String>(256);
    out_tx.send(subscribe_frame(topic, secret)).await.ok();
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

    // the loop's outcome, not a steering flag: `Some` means the node refused
    // this attach's topic, which every other exit path is not.
    let mut refused = None;
    loop {
        tokio::select! {
            frame = ws_rx.next() => {
                let Some(Ok(message)) = frame else { break };
                if message.is_close() {
                    break;
                }
                if let Message::Text(text) = message {
                    // the node refused the subscription — this attach can never
                    // receive a byte, and every keystroke it sends is dropped.
                    // Ctrl-C is a keystroke, so without this the terminal is
                    // black and unkillable until SIGTERM: the same wedge class
                    // the `term_ended` signal below closed, through a path the
                    // ws topic gate made reachable for the first time.
                    if let Some(detail) = topic_refusal(&text, topic) {
                        refused = Some(detail);
                        break;
                    }
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
    match refused {
        Some(detail) => Err(detail.into()),
        None => Ok(()),
    }
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

fn cmd_sched(args: SchedArgs, base: &str) -> AgentResult {
    let provider = resolve_provider(base, args.provider, Some(&args.cred))?;
    let tag = provider.token();

    let target = match args.host_node.as_deref() {
        Some(name) => resolve_host_node(base, name)?.to_vec(),
        None => own_node_key(base)?,
    };
    preflight_provider(base, &target, tag)?;

    let dispatch_id = fresh_dispatch_id();
    // saga's id space is namespaced per trigger origin, and `/v1/submit`
    // re-signs with THIS node's key — so the run's id lives under this node's
    // own actor namespace and no other member can create or squat it.
    let saga_id = saga::namespaced_id(
        &sdk::Origin::External(own_node_key(base)?),
        &format!("sched\u{1f}{dispatch_id}"),
    );
    let payload =
        compute_service::envelope::compose_headless(&saga_id, &args.prompt, Some(&args.cred))
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

    submit(base, "saga", serde_json::to_value(&trigger)?)?;
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

/// A fresh dispatch id: 32 random bytes as 64 hex chars — what
/// `run-output:<id>` keys on.
///
/// The WIDTH is a wire contract, not a taste call. A run's live output reaches
/// the node's ring through the ws `run_output` frame, whose admission gate
/// (`bin/noded/src/stream.rs`) accepts an id of EXACTLY 64 ascii-hex and drops
/// anything else with `reason = "malformed_run_id"`; the agent data plane's
/// `valid_event` enforces the same shape before forwarding a line to a peer.
/// `runs::dispatch_id_for` — the chat-driven lane's id — is a hex sha256 and so
/// satisfies it by construction. This one used to mint 16 bytes, which meant
/// EVERY `ducktape agent sched` run had its live output silently dropped at the
/// node while the committed result landed fine: the ring looked empty for a run
/// that plainly succeeded. Pinned by
/// [`tests::a_fresh_dispatch_id_is_a_wire_admissible_run_id`].
fn fresh_dispatch_id() -> String {
    let mut bytes = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut bytes);
    hex_bytes(&bytes)
}

// ============================================================================
// shared resolution
// ============================================================================


/// the service-link secret of the node this CLI is dialling — what its ws
/// surface admits a session's `term:<id>` topic against.
///
/// Reading it is the whole proof: the file is 0600 beside `node.toml`, so a
/// caller that can read it is the operator of that node — the same bar the node
/// key already sets, and the same secret the agent daemon presents to attach.
/// The DIRECTORY comes from the shared addressing ladder
/// ([`NodeAddr::workspace`]), so "which node" is answered once for both the url
/// this CLI dials and the files behind it.
fn workspace_secret(addr: &NodeAddr) -> Result<String, Box<dyn std::error::Error>> {
    let workspace = addr
        .workspace()
        .map_err(|why| format!("attaching a pty needs this node's workspace: {why}"))?;
    noded::services::read_link_token(&workspace).map_err(Into::into)
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
    let record = query_credential(base, name)?
        .ok_or_else(|| format!("unknown credential {name:?} — {}", credential_hint(base)))?;
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

/// What to say after "unknown credential": the names that DO exist, or the
/// command that makes the first one.
///
/// Best-effort by construction — this only ever runs on a path that is already
/// failing, so a second query that also fails must not replace the real error
/// with its own. It then says the one thing that is true regardless.
fn credential_hint(base: &str) -> String {
    const REGISTER_ONE: &str = "register one with: ducktape user cred add claude";
    let Ok(records) = crate::cred_cli::list_credential_names(base) else {
        return REGISTER_ONE.into();
    };
    match records.as_slice() {
        [] => format!("no credentials are registered on this node — {REGISTER_ONE}"),
        names => format!("registered here: {}", names.join(", ")),
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

/// the subscribe that ADMITS this connection to a session's output topic. The
/// `token` is the node's own 0600 service-link secret; without it the node
/// refuses the topic and this connection has nothing to send keystrokes on.
fn subscribe_frame(topic: &str, token: &str) -> String {
    serde_json::json!({ "op": "subscribe", "topics": [topic], "token": token }).to_string()
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

/// The node's refusal of THIS attach's topic, if that is what this frame is.
///
/// `ServerFrame::Error` is `{type:"error", topic, code, detail}`; the `detail`
/// is the node's own sentence and reaches the operator verbatim, like every
/// other refusal string this CLI surfaces.
///
/// Matched on the topic, not just the type: an error about some other topic on
/// a shared connection is not this attach's business. `agent pty` holds one
/// topic, but keying on it is what keeps that true.
fn topic_refusal(text: &str, topic: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let is_our_refusal =
        value["type"].as_str() == Some("error") && value["topic"].as_str() == Some(topic);
    if !is_our_refusal {
        return None;
    }
    let detail = value["detail"]
        .as_str()
        .unwrap_or("the node refused this session's topic");
    Some(detail.to_string())
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

    /// A sched run's id must be admissible on the wire that carries its LIVE
    /// output, or the ring stays empty for a run that succeeded. The node's ws
    /// `run_output` gate takes exactly 64 ascii-hex; so does the agent data
    /// plane's peer forwarder. See [`fresh_dispatch_id`].
    #[test]
    fn a_fresh_dispatch_id_is_a_wire_admissible_run_id() {
        let id = fresh_dispatch_id();
        assert_eq!(id.len(), 64, "the ws run_output gate drops any other width");
        assert!(
            id.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "the gate also requires ascii-hex: {id}"
        );
        assert_ne!(id, fresh_dispatch_id(), "a fresh id is fresh");
    }

    /// `sched` composes the run's saga id under the SUBMITTING node's own actor
    /// namespace, because `/v1/submit` re-signs with that key and saga refuses a
    /// trigger for anybody else's namespace. Composing a bare `sched\x1f<id>`
    /// here (what this used to do) would make every scheduled run reject.
    #[test]
    fn a_sched_saga_id_is_owned_by_the_submitting_node() {
        let node = sdk::Origin::External(vec![0xAB; 32]);
        let id = saga::namespaced_id(&node, &format!("sched\u{1f}{}", fresh_dispatch_id()));
        assert!(saga::owns_id(&node, &id), "saga would refuse {id:?}");
        assert!(
            !saga::owns_id(&sdk::Origin::Module("dispatch".into()), &id),
            "and it belongs to nobody else"
        );
    }

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

    /// The node's refusal must END the attach, not be swallowed.
    ///
    /// By the time a topic refusal can arrive, `cmd_pty` has already created a
    /// real pty holding a lent credential, entered raw mode and printed
    /// "attached". Every keystroke after that is dropped by the node — Ctrl-C
    /// included, since it is just a keystroke — so ignoring this frame is a
    /// black, unkillable terminal, the same wedge class the `term_ended` signal
    /// closed. The ws topic gate is what made this frame reachable for `term:`
    /// at all, so the handler ships with it.
    #[test]
    fn a_topic_refusal_ends_the_attach_and_carries_the_nodes_own_sentence() {
        let refusal = serde_json::json!({
            "type": "error",
            "topic": "term:abc",
            "code": "forbidden",
            "detail": "this topic requires the node's service-link token",
        })
        .to_string();
        assert_eq!(
            topic_refusal(&refusal, "term:abc").as_deref(),
            Some("this topic requires the node's service-link token"),
            "the node's sentence must reach the operator verbatim"
        );

        // an error about ANOTHER topic is not this attach's business ...
        assert_eq!(topic_refusal(&refusal, "term:other"), None);
        // ... and no ordinary frame is mistaken for one. A term chunk in
        // particular rides `type:"event"` on the very same topic.
        for benign in [
            serde_json::json!({ "type": "event", "topic": "term:abc", "item": "aGk=" }),
            serde_json::json!({ "type": "term_ended", "topic": "term:abc" }),
            serde_json::json!({ "type": "subscribed", "topics": { "term:abc": "0" } }),
            serde_json::json!({ "type": "heartbeat", "height": 1 }),
        ] {
            assert_eq!(
                topic_refusal(&benign.to_string(), "term:abc"),
                None,
                "{benign}"
            );
        }
        assert_eq!(topic_refusal("not json", "term:abc"), None);

        // a refusal with no detail still ends the attach rather than wedging it.
        let bare = serde_json::json!({ "type": "error", "topic": "term:abc" }).to_string();
        assert!(topic_refusal(&bare, "term:abc").is_some());
    }

    #[test]
    fn client_frames_carry_the_snake_case_op_tags() {
        let sub: serde_json::Value =
            serde_json::from_str(&subscribe_frame("term:x", "s3cr3t")).unwrap();
        assert_eq!(sub["op"], "subscribe");
        assert_eq!(sub["topics"][0], "term:x");
        // the field the node's topic gate reads. Without it the node refuses
        // `term:` and this client has nothing to send keystrokes on, so its
        // absence is a broken pty, not a cosmetic omission.
        assert_eq!(sub["token"], "s3cr3t");

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
