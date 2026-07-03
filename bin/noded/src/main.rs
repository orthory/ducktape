//! the ducktape node daemon: ONE embedded host behind http/ws.
//!
//! two runtimes, one thread each way: the node actor owns the (non-Send)
//! `host::Host` inside a commonware `tokio::Runner` — the qmdb-backed modules
//! need its storage context — and drains [`NodeCommand`]s in arrival order, one
//! msg per block. the axum server runs on a plain tokio runtime on the main
//! thread and only ever talks to the actor over the command channel. every app
//! build is a client: the web build dials this directly; the desktop shell
//! spawns it detached (an orphan — it outlives the window) and connects the
//! same way. POST /v1/shutdown is how a client retires it: no pid handshake,
//! the port IS the daemon's identity.
//!
//! run: `cargo run -p noded -- [--listen 127.0.0.1:8844] [--storage <dir>]`
//!
//! without `--storage` state lives in a fresh temp dir (clean run each boot).
//! with it, qmdb modules and the forge repo persist; the height counter still
//! restarts at 0 — it is a local block counter, not consensus state.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent::AgentModule;
use agent_oracle::{AuthStore, LlmWorker};
use automations::Automations;
use chat::Chat;
use commonware_runtime::{Runner as _, Supervisor as _};
use document::Document;
use files::Files;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, DispatchRecord, Host, SubmitError};
use inbox::Inbox;
use jobs::Jobs;
use memory::Memory;
use noded::{
    BlockSummary, DispatchInfo, ModuleStatus, NodeCommand, NodeHandle, NodeStatus, TelemetryEvent,
    TelemetryFrame, TelemetryRing, WsFrame, hex_root,
};
use profiles::Profiles;
use reactor::MAX_WORKER_ROUNDS;
use saga::SagaModule;
use sdk::{Effect, Event, Msg, Origin};
use tasks::Tasks;
use tokio::sync::broadcast;

/// every module registered at genesis, in registry order. status reports use
/// this list; keep it in sync with the genesis vec in `run_node`.
const MODULE_IDS: [&str; 12] = [
    "chat",
    "saga",
    "tasks",
    "inbox",
    "automations",
    "jobs",
    "agent",
    "document",
    "forge",
    "files",
    "memory",
    "profiles",
];
const ORACLE_ORIGIN: &[u8] = b"oracle";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen: SocketAddr = "127.0.0.1:8844".parse()?;
    let mut storage: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("--listen needs an addr")?.parse()?,
            "--storage" => storage = args.next().map(PathBuf::from),
            other => {
                return Err(format!("unexpected arg {other:?} (want --listen/--storage)").into());
            }
        }
    }
    let storage = storage.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("ducktape-noded-{}", std::process::id()))
    });
    // forge's on-disk repo base, derived ONCE: the node actor's `Forge` and the
    // http git upload-pack (clone) lane must agree on it, so both are handed the
    // same path — the actor to materialize into, the http handle to serve from.
    let forge_repo = storage.join("forge-git");

    let (handle, cmd_rx, event_tx) = NodeHandle::channel();
    let handle = handle.with_forge_repo(forge_repo.clone());

    // the node actor gets its own thread: commonware's tokio runner owns that
    // thread's runtime, and the host must never leave it. the blob handle and
    // the telemetry ring are the node-local surfaces that cross: the actor
    // registers the files module over the blobs and pushes per-block telemetry
    // into the ring, while the http layer reads both through its own clones.
    let actor_storage = storage.clone();
    let actor_forge_repo = forge_repo.clone();
    let blobs = handle.blob_handle();
    let telemetry = handle.telemetry_ring();
    std::thread::Builder::new()
        .name("node-actor".into())
        .spawn(move || {
            run_node(
                actor_storage,
                actor_forge_repo,
                blobs,
                telemetry,
                cmd_rx,
                event_tx,
            )
        })?;

    println!(
        "[noded] listening on {listen}, storage {}",
        storage.display()
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(listen).await?;
            noded::serve(listener, handle).await?;
            // in-flight requests drained; blocks commit at the block boundary,
            // so exiting here loses nothing.
            println!("[noded] shutdown requested, exiting");
            Ok(())
        })
}

/// own the host for the process lifetime: genesis the module set, then apply
/// commands in arrival order — every submit is its own block.
fn run_node(
    storage: PathBuf,
    forge_repo: PathBuf,
    blobs: files::BlobHandle,
    telemetry: TelemetryRing,
    mut cmds: mpsc::Receiver<NodeCommand>,
    events: broadcast::Sender<WsFrame>,
) {
    // forge_repo is derived by the caller (shared with the http upload-pack lane).
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: the full product surface. chat/tasks/inbox as the core loop,
        // automations bridging chat/memory events into chat/tasks/inbox
        // follow-ups, jobs for deferred work, document + forge for the
        // substrate-backed stores, and files + memory for the content planes.
        // files registers over the
        // http layer's blob handle so uploads land in the store `serve_sync`
        // reads — the bytes themselves never touch consensus.
        let chat = Chat::init(context.child("chat"), "chat").await;
        let saga = SagaModule::new("saga");
        let tasks = Tasks::new("tasks");
        let inbox = Inbox::new("inbox");
        let automations = Automations::new("automations", "chat", "tasks", "inbox", "memory");
        let jobs = Jobs::new("jobs");
        let agent = AgentModule::new(
            "agent",
            "chat",
            "saga",
            Some("tasks".into()),
            Some("jobs".into()),
        );
        let document = Document::init(context.child("document"), "document").await;
        // forge shares the files body plane so a Push's packfile — uploaded to
        // the blob lane before the op is submitted — materializes locally; the
        // pack bytes never enter consensus (root stays sha256(head oid)).
        let forge = Forge::with_blobs("forge", forge_repo, blobs.clone()).expect("forge init");
        let worker_blobs = blobs.clone();
        let files = Files::with_blobs("files", blobs);
        let memory = Memory::new("memory", "files");
        // the origin-gated display-name registry: maps each verified submit
        // origin to a chosen name so the ui can resolve authors to names.
        let profiles = Profiles::new("profiles");
        let mut host = Host::genesis(vec![
            Box::new(chat),
            Box::new(saga),
            Box::new(tasks),
            Box::new(inbox),
            Box::new(automations),
            Box::new(jobs),
            Box::new(agent),
            Box::new(document),
            Box::new(forge),
            Box::new(files),
            Box::new(memory),
            Box::new(profiles),
        ])
        .expect("genesis");

        println!("[noded] genesis app_hash={}", hex_root(&host.app_hash()));

        let workers = oracle_workers(worker_blobs);
        let mut height = 0u64;
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    let result = submit_and_drain(
                        &mut host,
                        &workers,
                        &mut height,
                        &events,
                        &telemetry,
                        Origin::External(origin),
                        Msg { target, payload },
                    )
                    .await;
                    let _ = reply.send(result); // caller may have hung up
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host
                        .query(&target, &req)
                        .await
                        .map_err(|err| err.to_string());
                    let _ = reply.send(result);
                }
                NodeCommand::Status { reply } => {
                    let modules = MODULE_IDS
                        .iter()
                        .map(|id| ModuleStatus {
                            id: (*id).into(),
                            root: host
                                .module_root(id)
                                .map(|root| hex_root(&root))
                                .unwrap_or_default(),
                        })
                        .collect();
                    let _ = reply.send(NodeStatus {
                        version: env!("CARGO_PKG_VERSION").into(),
                        app_hash: hex_root(&host.app_hash()),
                        height,
                        modules,
                    });
                }
            }
        }
    });
}

/// wall-clock millis for `consensus_time`. a single-writer local block counter
/// has no consensus clock to agree on; wall time keeps module timestamps
/// (message sent_at, task created_at) meaningful to the ui.
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is past the epoch")
        .as_millis() as u64
}

fn oracle_workers(blobs: files::BlobHandle) -> Vec<Box<dyn reactor::Worker>> {
    #[cfg(debug_assertions)]
    {
        if std::env::var_os("DUCKTAPE_NODED_ECHO_ORACLE").is_some() {
            return vec![Box::new(EchoWorker)];
        }
    }
    vec![Box::new(LlmWorker::new(
        blobs,
        AuthStore::from_default_path(),
        // the ChatGPT/Codex subscription endpoint rejects gpt-5.1 (400 "not
        // supported when using Codex with a ChatGPT account") — default to a
        // model the account can serve; per-agent model_ref overrides this.
        "gpt-5.3-codex-spark".into(),
    ))]
}

async fn submit_and_drain(
    host: &mut Host,
    workers: &[Box<dyn reactor::Worker>],
    height: &mut u64,
    events: &broadcast::Sender<WsFrame>,
    telemetry: &TelemetryRing,
    origin: Origin,
    msg: Msg,
) -> Result<BlockSummary, String> {
    let (mut last, effects) = match submit_one(host, height, events, telemetry, origin, msg).await {
        Ok(out) => out,
        Err(SubmitError::Fatal(err)) => {
            eprintln!("[noded] FATAL: {err} — halting");
            std::process::exit(1);
        }
        Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
    };

    let mut queue = VecDeque::new();
    offer_effects(workers, effects, &mut queue).await;
    let mut rounds = 1u32;

    while let Some(follow) = queue.pop_front() {
        rounds += 1;
        if rounds > MAX_WORKER_ROUNDS {
            return Err("worker-round budget exceeded".into());
        }
        match submit_one(
            host,
            height,
            events,
            telemetry,
            Origin::External(ORACLE_ORIGIN.to_vec()),
            follow,
        )
        .await
        {
            Ok((block, effects)) => {
                last = block;
                offer_effects(workers, effects, &mut queue).await;
            }
            Err(SubmitError::Fatal(err)) => {
                eprintln!("[noded] FATAL: {err} — halting");
                std::process::exit(1);
            }
            Err(err @ SubmitError::Rejected(_)) => {
                eprintln!("[noded] worker follow-up rejected: {err}");
            }
        }
    }

    Ok(last)
}

async fn submit_one(
    host: &mut Host,
    height: &mut u64,
    events: &broadcast::Sender<WsFrame>,
    telemetry: &TelemetryRing,
    origin: Origin,
    msg: Msg,
) -> Result<(BlockSummary, Vec<Effect>), SubmitError> {
    let consensus_time = unix_millis();
    let ctx = BlockContext {
        height: *height + 1,
        consensus_time,
        origin,
    };
    // node-local wall-clock cost of applying this block — the telemetry plane's
    // one non-deterministic signal. measured HERE in the effectful daemon layer,
    // never inside the deterministic host, so the kernel stays clock-free.
    let started = Instant::now();
    let out = host.submit_at(ctx, msg).await?;
    let latency_us = started.elapsed().as_micros() as u64;
    *height += 1;

    let block = BlockSummary {
        height: *height,
        app_hash: hex_root(&out.app_hash),
    };
    let frame = TelemetryFrame {
        height: *height,
        consensus_time,
        latency_us,
        dispatches: out.dispatches.iter().map(dispatch_info).collect(),
        events: out.events.iter().map(event_preview).collect(),
    };
    // buffer for GET /v1/telemetry, then fan both frames out live. no subscribers
    // is fine — send only fails then.
    telemetry.push(frame.clone());
    let _ = events.send(WsFrame::Block(block.clone()));
    let _ = events.send(WsFrame::Telemetry(frame));
    Ok((block, out.effects))
}

/// map a deterministic dispatch record to its telemetry wire twin.
fn dispatch_info(record: &DispatchRecord) -> DispatchInfo {
    DispatchInfo {
        module: record.module.clone(),
        origin: origin_tag(&record.origin),
        emitted_msgs: record.emitted_msgs,
        emitted_events: record.emitted_events,
    }
}

/// a compact, human-legible tag for what triggered a dispatch.
fn origin_tag(origin: &Origin) -> String {
    match origin {
        Origin::External(name) if name.is_empty() => "external".to_string(),
        Origin::External(name) => format!("external:{}", String::from_utf8_lossy(name)),
        Origin::Module(id) => format!("module:{id}"),
        Origin::System => "system".to_string(),
    }
}

/// best-effort text preview of an emitted event's (module-defined) payload,
/// capped so a large payload can't bloat a telemetry frame.
fn event_preview(ev: &Event) -> TelemetryEvent {
    const PREVIEW_CAP: usize = 512;
    let end = ev.payload.len().min(PREVIEW_CAP);
    TelemetryEvent {
        source: ev.source.clone(),
        payload: String::from_utf8_lossy(&ev.payload[..end]).into_owned(),
    }
}

async fn offer_effects(
    workers: &[Box<dyn reactor::Worker>],
    effects: Vec<Effect>,
    queue: &mut VecDeque<Msg>,
) {
    for eff in effects {
        let mut claimed = false;
        for w in workers {
            match w.run(&eff).await {
                Ok(Some(follow)) => {
                    queue.push_back(follow);
                    claimed = true;
                    break;
                }
                Ok(None) => {}
                Err(err) => {
                    eprintln!("[noded] worker error: {err}");
                    claimed = true;
                    break;
                }
            }
        }
        if !claimed {
            println!(
                "[noded] effect with no worker ({} bytes) — dropped",
                eff.0.len()
            );
        }
    }
}

#[cfg(debug_assertions)]
struct EchoWorker;

#[cfg(debug_assertions)]
#[async_trait::async_trait(?Send)]
impl reactor::Worker for EchoWorker {
    async fn run(&self, effect: &Effect) -> Result<Option<Msg>, reactor::Error> {
        let request = match saga_interface::decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(None),
        };
        let llm = match agent_interface::decode_llm_request(&request.spec) {
            Ok(llm) => llm,
            Err(_) => return Ok(None),
        };
        Ok(Some(Msg {
            target: "saga".into(),
            payload: saga_interface::encode_msg(&saga_interface::SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome: Ok(agent_interface::encode_output(
                    &agent_interface::AgentOutput {
                        reply_blocks: vec![chat_interface::Block::paragraph(format!(
                            "echo: handling {}",
                            llm.run_id
                        ))],
                        actions: Vec::new(),
                    },
                )),
            }),
        }))
    }
}
