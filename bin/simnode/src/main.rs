//! the deterministic scenario node: noded's exact /v1 http+ws wire over a
//! test-scripted block producer.
//!
//! same two-runtime shape as noded — the (non-Send) `host::Host` lives on its
//! own actor thread inside a commonware runner, axum serves on the main
//! thread — but the actor answers to TWO channels: the [`NodeCommand`] lane
//! every client dials, and a `/sim/*` control lane only tests use. the
//! semantic difference is WHEN blocks commit:
//!
//!   - hold mode (default): a submit is queued and its http reply retained —
//!     the request hangs, exactly like the networked validator's held submit —
//!     until `POST /sim/step` commits the next queued op as one block.
//!   - auto mode (`--auto` or `POST /sim/auto`): every submit commits
//!     immediately and drains its worker follow-ups, i.e. noded's behavior.
//!
//! block time is a LOGICAL clock (`SIM_EPOCH_MS + height * SIM_BLOCK_MS`) —
//! the one wall-clock read in noded's block path (`consensus_time`) is what
//! made replays non-reproducible, so the same op script always produces the
//! same app-hash here. reads (query/status) always serve committed state:
//! held ops are invisible until stepped, which is consensus semantics.
//!
//! the blocks/index lane is the real daemons' exactly: every commit feeds the
//! durable block index (`BlockOps.record` via [`noded::block_row`]), so
//! `GET /v1/blocks` and `/v1/index/*` serve just like noded. personas shape
//! the one wire difference left between the two real nodes — the receipt:
//!   - `local`: submit receipts carry `opHash` (the embedded daemon's shape).
//!   - `networked`: receipts are height-only — a response layer strips
//!     `opHash`, the validator's shape until its ordered-node convergence.
//!
//! `POST /sim/peer-block` commits a block owned by no held submit — the
//! "concurrent writer" for optimistic-projection race scenarios.
//!
//! honesty rules: no synthetic-rejection knob (rejection scenarios must use
//! genuinely rejectable ops, so module semantics stay real), no LlmWorker
//! (an external llm call in a determinism tool is a contradiction; the echo
//! worker behind `--echo-oracle` is the only oracle). storage should be a
//! fresh dir per run (the height resumes above the index watermark like
//! noded's, but reused module state defeats the same-script reproducibility
//! this tool exists for).
//!
//! run: `cargo run -p simnode -- [--listen 127.0.0.1:8845] [--storage <dir>]
//!       [--auto] [--persona local|networked] [--echo-oracle]`
//!
//! v1 limit, by design: a rejected op never becomes a block here
//! (Host::submit_at aborts it pre-commit; only the ordered validator
//! journals rejected frames as blocks).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use agent::AgentModule;
use automations::Automations;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use chat::Chat;
use commonware_runtime::{Metrics as _, Runner as _, Supervisor as _};
use dispatch::DispatchModule;
use tagging::TaggingModule;
use document::Document;
use files::Files;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use futures::select;
use host::{BlockContext, DispatchRecord, Host, SubmitError};
use inbox::Inbox;
use indexer::{AppliedOp, BlockOps, IndexStore, OriginTag};
use jobs::Jobs;
use memory::Memory;
use noded::{
    BlockDisposition, BlockRecord, BlockSummary, DispatchInfo, ModuleCategory, ModuleStatus,
    NodeCommand, NodeHandle, NodeStatus, TelemetryEvent, TelemetryFrame, TelemetryRing, WsFrame,
    block_row, hex_bytes, hex_root, payload_preview,
};
use pages::Pages;
use profiles::Profiles;
use reactor::MAX_WORKER_ROUNDS;
use saga::SagaModule;
use sdk::{Effect, Event, Msg, Origin};
use serde::{Deserialize, Serialize};
use tasks::Tasks;
use tokio::sync::broadcast;

/// every module registered at genesis, in registry order — noded's exact set,
/// so status/roots and query targets match what the app expects of a daemon.
const MODULE_IDS: [&str; 15] = [
    "chat",
    "saga",
    "dispatch",
    "tagging",
    "tasks",
    "inbox",
    "automations",
    "jobs",
    "agent",
    "document",
    "pages",
    "forge",
    "files",
    "memory",
    "profiles",
];
const ORACLE_ORIGIN: &[u8] = b"oracle";
const PEER_ORIGIN: &[u8] = b"peer";

/// the logical clock: `consensus_time = SIM_EPOCH_MS + height * SIM_BLOCK_MS`.
/// a fixed epoch keeps module timestamps (message sent_at, task created_at)
/// plausible in the ui while staying identical across runs.
const SIM_EPOCH_MS: u64 = 1_750_000_000_000;
const SIM_BLOCK_MS: u64 = 1_000;

/// cap when buffering a /v1/submit response body to strip `opHash` — receipts
/// are ~200 bytes; anything past this is not a receipt.
const RECEIPT_BODY_CAP: usize = 64 * 1024;

// ── Wire shapes of the /sim control lane ────────────────

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Persona {
    Local,
    Networked,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SimSnapshot {
    height: u64,
    held: usize,
    oracle_queued: usize,
    auto: bool,
    persona: Persona,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CommittedInfo {
    height: u64,
    app_hash: String,
    op_hash: String,
    target: String,
    /// `held` (a client submit released by this step), `oracle` (a worker
    /// follow-up), or `peer` (a /sim/peer-block).
    kind: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StepReport {
    /// `null` when the step found both queues empty, or when the stepped op
    /// was rejected (its submitter got the rejection as its reply).
    committed: Option<CommittedInfo>,
    #[serde(flatten)]
    snapshot: SimSnapshot,
}

#[derive(Deserialize)]
struct AutoRequest {
    enabled: bool,
}

#[derive(Deserialize)]
struct PersonaRequest {
    persona: Persona,
}

#[derive(Deserialize)]
struct PeerBlockRequest {
    target: String,
    payload: serde_json::Value,
    origin: Option<String>,
}

// ── Control commands into the actor ─────────────────────

enum SimCommand {
    Step {
        reply: oneshot::Sender<StepReport>,
    },
    SetAuto {
        enabled: bool,
        reply: oneshot::Sender<SimSnapshot>,
    },
    SetPersona {
        persona: Persona,
        reply: oneshot::Sender<SimSnapshot>,
    },
    PeerBlock {
        target: String,
        payload: Vec<u8>,
        origin: Vec<u8>,
        reply: oneshot::Sender<Result<CommittedInfo, String>>,
    },
    Snapshot {
        reply: oneshot::Sender<SimSnapshot>,
    },
}

/// the control lane's axum state: a sender into the actor. persona lives
/// separately (shared with the receipt-strip layer) because the middleware
/// must read it per-response without an actor round-trip.
#[derive(Clone)]
struct SimHandle {
    control: mpsc::Sender<SimCommand>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen: SocketAddr = "127.0.0.1:8845".parse()?;
    let mut storage: Option<PathBuf> = None;
    let mut auto = false;
    let mut persona = Persona::Local;
    let mut echo_oracle = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("--listen needs an addr")?.parse()?,
            "--storage" => storage = args.next().map(PathBuf::from),
            "--auto" => auto = true,
            "--persona" => {
                persona = match args.next().as_deref() {
                    Some("local") => Persona::Local,
                    Some("networked") => Persona::Networked,
                    other => {
                        return Err(format!("--persona wants local|networked, got {other:?}").into());
                    }
                }
            }
            "--echo-oracle" => echo_oracle = true,
            other => {
                return Err(format!(
                    "unexpected arg {other:?} (want --listen/--storage/--auto/--persona/--echo-oracle)"
                )
                .into());
            }
        }
    }
    let storage = storage.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("ducktape-simnode-{}", std::process::id()))
    });
    let forge_repo = storage.join("forge-git");
    let persona_label = format!("{persona:?}");

    // the durable block index: /v1/blocks and /v1/index/* read it, the sim
    // actor feeds it block-by-block — same wiring as the real daemons.
    let index_dir = storage.join("index");
    let index = IndexStore::open(&index_dir, &MODULE_IDS)
        .map(|store| {
            Arc::new(
                store
                    .with_indexer(Box::new(chat_index::ChatIndex::new("chat")))
                    .with_indexer(Box::new(tasks_index::TasksIndex::new("tasks")))
                    .with_indexer(Box::new(document_index::DocumentIndex::new("document")))
                    .with_indexer(Box::new(pages_index::PagesIndex::new("pages"))),
            )
        })
        .map_err(|err| {
            format!(
                "open module index at {}: {err} (derived tier — delete the directory to rebuild)",
                index_dir.display()
            )
        })?;

    let (handle, cmd_rx, event_tx) = NodeHandle::channel();
    let handle = handle
        .with_forge_repo(forge_repo.clone())
        .with_index_store(index.clone());
    let (control_tx, control_rx) = mpsc::channel::<SimCommand>(16);
    let persona = Arc::new(Mutex::new(persona));

    let actor_storage = storage.clone();
    let actor_persona = persona.clone();
    let blobs = handle.blob_handle();
    let telemetry = handle.telemetry_ring();
    std::thread::Builder::new()
        .name("sim-actor".into())
        .spawn(move || {
            run_sim(
                actor_storage,
                forge_repo,
                index,
                blobs,
                telemetry,
                actor_persona,
                auto,
                echo_oracle,
                cmd_rx,
                control_rx,
                event_tx,
            )
        })?;

    println!(
        "[simnode] listening on {listen}, storage {}, hold={}, persona={persona_label}",
        storage.display(),
        !auto,
    );
    let sim_handle = SimHandle {
        control: control_tx,
    };
    let shutdown = handle.clone();
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(listen).await?;
            let app = noded::router(handle).merge(sim_router(sim_handle)).layer(
                axum::middleware::from_fn_with_state(persona, strip_receipt_op_hash),
            );
            axum::serve(listener, app)
                .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
                .await?;
            println!("[simnode] shutdown requested, exiting");
            Ok(())
        })
}

// ── The actor ───────────────────────────────────────────

/// a client submit parked until a step commits it: the retained reply is what
/// keeps its http request hanging — the held-submit semantics under test.
struct HeldOp {
    origin: Vec<u8>,
    msg: Msg,
    reply: oneshot::Sender<Result<BlockSummary, String>>,
}

struct Committed {
    block: BlockSummary,
    op_hash: String,
    target: String,
}

struct Sim {
    host: Host,
    height: u64,
    auto: bool,
    persona: Arc<Mutex<Persona>>,
    held: VecDeque<HeldOp>,
    /// worker follow-ups awaiting commits. steps drain this BEFORE the next
    /// held submit — noded drains a submit's follow-ups to completion before
    /// touching the next command, and step order mirrors that.
    oracle_queue: VecDeque<Msg>,
    workers: Vec<Box<dyn reactor::Worker>>,
    blobs: files::BlobHandle,
    telemetry: TelemetryRing,
    index: Arc<IndexStore>,
    events: broadcast::Sender<WsFrame>,
}

#[allow(clippy::too_many_arguments)]
fn run_sim(
    storage: PathBuf,
    forge_repo: PathBuf,
    index: Arc<IndexStore>,
    blobs: files::BlobHandle,
    telemetry: TelemetryRing,
    persona: Arc<Mutex<Persona>>,
    auto: bool,
    echo_oracle: bool,
    mut cmds: mpsc::Receiver<NodeCommand>,
    mut control: mpsc::Receiver<SimCommand>,
    events: broadcast::Sender<WsFrame>,
) {
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: noded's exact module set (keep in sync with MODULE_IDS) so
        // app queries and status roots behave like a real daemon's.
        let chat = Chat::init(context.child("chat"), "chat")
            .await
            .with_tagging("tagging");
        let saga = SagaModule::new("saga");
        let dispatch = DispatchModule::new("dispatch", "saga");
        let tagging = TaggingModule::new("tagging");
        let tasks = Tasks::new("tasks");
        let inbox = Inbox::new("inbox");
        let automations = Automations::new("automations", "chat", "tasks", "inbox", "memory");
        let jobs = Jobs::new("jobs");
        let agent = AgentModule::new(
            "agent",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            Some("tasks".into()),
            Some("jobs".into()),
            Some("document".into()),
        );
        let document = Document::init(context.child("document"), "document").await;
        let pages = Pages::init(context.child("pages"), "pages").await;
        let forge = Forge::with_blobs("forge", forge_repo, blobs.clone()).expect("forge init");
        let files = Files::with_blobs("files", blobs.clone());
        let memory = Memory::new("memory", "files");
        let profiles = Profiles::new("profiles");
        let host = Host::genesis(vec![
            Box::new(chat),
            Box::new(saga),
            Box::new(dispatch),
            Box::new(tagging),
            Box::new(tasks),
            Box::new(inbox),
            Box::new(automations),
            Box::new(jobs),
            Box::new(agent),
            Box::new(document),
            Box::new(pages),
            Box::new(forge),
            Box::new(files),
            Box::new(memory),
            Box::new(profiles),
        ])
        .expect("genesis");

        println!("[simnode] genesis app_hash={}", hex_root(&host.app_hash()));

        // resume above the index watermark like noded — with the contractual
        // fresh dir this is 0; on a (discouraged) reused dir it keeps op-log
        // heights monotonic instead of silently skipping every new block.
        let height = index.resume_height().expect("read index watermarks");

        let mut sim = Sim {
            host,
            height,
            auto,
            persona,
            held: VecDeque::new(),
            oracle_queue: VecDeque::new(),
            workers: if echo_oracle {
                vec![Box::new(EchoWorker)]
            } else {
                Vec::new()
            },
            blobs,
            telemetry,
            index,
            events,
        };

        loop {
            select! {
                cmd = control.next() => match cmd {
                    Some(cmd) => sim.handle_control(cmd).await,
                    None => break,
                },
                cmd = cmds.next() => match cmd {
                    Some(NodeCommand::Submit { target, payload, origin, reply }) => {
                        sim.handle_submit(origin, Msg { target, payload }, reply).await;
                    }
                    Some(NodeCommand::Query { target, req, reply }) => {
                        let result = sim.host.query(&target, &req).await.map_err(|err| err.to_string());
                        let _ = reply.send(result);
                    }
                    Some(NodeCommand::Status { reply }) => {
                        let _ = reply.send(sim.status());
                    }
                    Some(NodeCommand::Metrics { reply }) => {
                        let _ = reply.send(context.encode());
                    }
                    None => break,
                },
            }
        }
    });
}

impl Sim {
    async fn handle_submit(
        &mut self,
        origin: Vec<u8>,
        msg: Msg,
        reply: oneshot::Sender<Result<BlockSummary, String>>,
    ) {
        if !self.auto {
            self.held.push_back(HeldOp { origin, msg, reply });
            return;
        }
        // auto mode = noded's submit_and_drain: commit the caller's op, then
        // drain its worker follow-ups to completion, each its own block.
        let result = self.commit(Origin::External(origin), msg).await;
        let result = match result {
            Ok(committed) => match self.drain_oracle_budgeted().await {
                Ok(()) => Ok(committed.block),
                Err(err) => Err(err),
            },
            Err(err) => Err(err),
        };
        let _ = reply.send(result); // caller may have hung up
    }

    async fn handle_control(&mut self, cmd: SimCommand) {
        match cmd {
            SimCommand::Step { reply } => {
                let committed = self.step_once().await;
                let _ = reply.send(StepReport {
                    committed,
                    snapshot: self.snapshot(),
                });
            }
            SimCommand::SetAuto { enabled, reply } => {
                self.auto = enabled;
                // entering auto flushes the backlog: every held op (and the
                // follow-ups it spawns) commits now, so "auto" always means
                // "nothing is parked".
                if enabled {
                    while self.step_once().await.is_some()
                        || !(self.held.is_empty() && self.oracle_queue.is_empty())
                    {
                    }
                }
                let _ = reply.send(self.snapshot());
            }
            SimCommand::SetPersona { persona, reply } => {
                *self.persona.lock().expect("persona poisoned") = persona;
                let _ = reply.send(self.snapshot());
            }
            SimCommand::PeerBlock {
                target,
                payload,
                origin,
                reply,
            } => {
                let result = self
                    .commit(Origin::External(origin), Msg { target, payload })
                    .await
                    .map(|committed| committed_info(&committed, "peer"));
                let _ = reply.send(result);
            }
            SimCommand::Snapshot { reply } => {
                let _ = reply.send(self.snapshot());
            }
        }
    }

    /// commit exactly one queued op — a pending oracle follow-up first, else
    /// the oldest held submit (releasing its receipt). None when idle or when
    /// the stepped op was rejected (the submitter got the rejection reply).
    async fn step_once(&mut self) -> Option<CommittedInfo> {
        if let Some(follow) = self.oracle_queue.pop_front() {
            return match self
                .commit(Origin::External(ORACLE_ORIGIN.to_vec()), follow)
                .await
            {
                Ok(committed) => Some(committed_info(&committed, "oracle")),
                Err(err) => {
                    eprintln!("[simnode] worker follow-up rejected: {err}");
                    None
                }
            };
        }
        let held = self.held.pop_front()?;
        let result = self.commit(Origin::External(held.origin), held.msg).await;
        let info = result
            .as_ref()
            .ok()
            .map(|committed| committed_info(committed, "held"));
        let _ = held.reply.send(result.map(|committed| committed.block));
        info
    }

    /// noded's follow-up budget, for auto mode only: manual steps are already
    /// bounded by the test issuing them one at a time.
    async fn drain_oracle_budgeted(&mut self) -> Result<(), String> {
        let mut rounds = 1u32;
        while let Some(follow) = self.oracle_queue.pop_front() {
            rounds += 1;
            if rounds > MAX_WORKER_ROUNDS {
                return Err("worker-round budget exceeded".into());
            }
            if let Err(err) = self
                .commit(Origin::External(ORACLE_ORIGIN.to_vec()), follow)
                .await
            {
                eprintln!("[simnode] worker follow-up rejected: {err}");
            }
        }
        Ok(())
    }

    /// the one commit point — noded's submit_one under the logical clock:
    /// apply the op at the next height, publish the same frames a real node
    /// publishes (telemetry ring, ws block + telemetry), stage the payload
    /// (op hash == content address), and feed the durable block index that
    /// serves GET /v1/blocks and /v1/index/*.
    async fn commit(&mut self, origin: Origin, msg: Msg) -> Result<Committed, String> {
        let target = msg.target.clone();
        let payload = payload_preview(&msg.payload);
        let proposer = proposer_hex(&origin);
        let consensus_time = SIM_EPOCH_MS + (self.height + 1) * SIM_BLOCK_MS;
        // staging IS hashing (put_chunk keys by sha256), and every real
        // surface stages committed payloads — the local daemon at submit, the
        // validator at drain — so /v1/files/blob/{opHash} dereferences here too.
        let op_hash = hex_bytes(&self.blobs.put_chunk(msg.payload.clone()));
        let ctx = BlockContext {
            protocol_version: 0,
            height: self.height + 1,
            consensus_time,
            origin,
        };
        let out = match self.host.submit_at(ctx, msg).await {
            Ok(out) => out,
            Err(SubmitError::Fatal(err)) => {
                // same fail-stop as the real daemons: a half-committed host is
                // indeterminate, and a SIM that limps past it would hand tests
                // green runs over corrupt state.
                eprintln!("[simnode] FATAL: {err} — halting");
                std::process::exit(1);
            }
            Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
        };
        self.height += 1;

        let block = BlockSummary {
            height: self.height,
            app_hash: hex_root(&out.app_hash),
        };
        let operations: Vec<DispatchInfo> = out.dispatches.iter().map(dispatch_info).collect();
        let frame = TelemetryFrame {
            height: self.height,
            consensus_time,
            // the real daemons measure wall-clock apply cost here — the
            // telemetry plane's one non-deterministic signal. the sim reports
            // zero instead of lying with a real measurement.
            latency_us: 0,
            dispatches: operations.clone(),
            events: out.events.iter().map(event_preview).collect(),
        };
        self.telemetry.push(frame.clone());
        let _ = self.events.send(WsFrame::Block(block.clone()));
        let _ = self.events.send(WsFrame::Telemetry(frame));

        // fold the block into the durable index LAST, like noded: canonical
        // state is already committed, so an index failure degrades the read
        // models, never the block. the frame hash stays empty — nothing is
        // framed on this lane, same discipline as the real daemon.
        let ops = out
            .dispatches
            .into_iter()
            .map(|d| AppliedOp {
                origin: index_origin(&d.origin),
                module: d.module,
                payload: d.payload,
            })
            .collect();
        let block_ops = BlockOps {
            height: self.height,
            time: consensus_time,
            ops,
            record: Some(block_row(&BlockRecord {
                height: self.height,
                hash: String::new(),
                commit_hash: block.app_hash.clone(),
                proposer,
                disposition: BlockDisposition::Applied,
                target: target.clone(),
                operations,
                payload,
                op_hash: op_hash.clone(),
            })),
        };
        if let Err(err) = self.index.apply_block(&block_ops) {
            eprintln!(
                "[simnode] module index apply failed at height {}: {err} — wipe <storage>/index to rebuild",
                self.height
            );
        }

        offer_effects(&self.workers, out.effects, &mut self.oracle_queue).await;
        Ok(Committed {
            block,
            op_hash,
            target,
        })
    }

    fn status(&self) -> NodeStatus {
        let modules = MODULE_IDS
            .iter()
            .map(|id| ModuleStatus {
                id: (*id).into(),
                root: self
                    .host
                    .module_root(id)
                    .map(|root| hex_root(&root))
                    .unwrap_or_default(),
                category: ModuleCategory::of(id),
            })
            .collect();
        NodeStatus {
            version: env!("CARGO_PKG_VERSION").into(),
            app_hash: hex_root(&self.host.app_hash()),
            height: self.height,
            modules,
        }
    }

    fn snapshot(&self) -> SimSnapshot {
        SimSnapshot {
            height: self.height,
            held: self.held.len(),
            oracle_queued: self.oracle_queue.len(),
            auto: self.auto,
            persona: *self.persona.lock().expect("persona poisoned"),
        }
    }
}

fn committed_info(committed: &Committed, kind: &'static str) -> CommittedInfo {
    CommittedInfo {
        height: committed.block.height,
        app_hash: committed.block.app_hash.clone(),
        op_hash: committed.op_hash.clone(),
        target: committed.target.clone(),
        kind,
    }
}

/// map a dispatch origin to the index's flattened origin tag — noded's exact
/// mapping, so index rows read identically across the real and sim daemons.
fn index_origin(origin: &Origin) -> OriginTag {
    match origin {
        Origin::External(id) => OriginTag::external(String::from_utf8_lossy(id)),
        Origin::Module(id) => OriginTag::module(id.clone()),
        Origin::System => OriginTag::system(),
    }
}

fn proposer_hex(origin: &Origin) -> String {
    match origin {
        Origin::External(key) => hex_bytes(key),
        Origin::Module(id) => format!("module:{id}"),
        Origin::System => "system".into(),
    }
}

/// map a deterministic dispatch record to its telemetry wire twin, keeping the
/// submitter's readable name (`external:<name>`) like noded's frames do — the
/// ring's `DispatchInfo::from` deliberately flattens to plain `external`.
fn dispatch_info(record: &DispatchRecord) -> DispatchInfo {
    DispatchInfo {
        module: record.module.clone(),
        origin: match &record.origin {
            Origin::External(name) if name.is_empty() => "external".to_string(),
            Origin::External(name) => format!("external:{}", String::from_utf8_lossy(name)),
            Origin::Module(id) => format!("module:{id}"),
            Origin::System => "system".to_string(),
        },
        emitted_msgs: record.emitted_msgs,
        emitted_events: record.emitted_events,
    }
}

/// best-effort text preview of an emitted event's payload, capped like noded's.
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
                Ok(reactor::WorkOutcome::Handled(follow)) => {
                    queue.extend(follow);
                    claimed = true;
                    break;
                }
                Ok(reactor::WorkOutcome::NotMine) => {}
                Err(err) => {
                    eprintln!("[simnode] worker error: {err}");
                    claimed = true;
                    break;
                }
            }
        }
        if !claimed {
            println!(
                "[simnode] effect with no worker ({} bytes) — dropped",
                eff.0.len()
            );
        }
    }
}

/// noded's debug echo oracle, unconditional here: the sim is a dev tool, and a
/// deterministic canned reply is the ONLY oracle that belongs in it.
struct EchoWorker;

#[async_trait::async_trait(?Send)]
impl reactor::Worker for EchoWorker {
    async fn run(&self, effect: &Effect) -> Result<reactor::WorkOutcome, reactor::Error> {
        let request = match saga_interface::decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(reactor::WorkOutcome::NotMine),
        };
        let llm = match agent_interface::decode_llm_request(&request.spec) {
            Ok(llm) => llm,
            Err(_) => return Ok(reactor::WorkOutcome::NotMine),
        };
        Ok(reactor::WorkOutcome::Handled(Some(Msg {
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
        })))
    }
}

// ── The /sim control routes ─────────────────────────────

fn sim_router(handle: SimHandle) -> Router {
    Router::new()
        .route("/sim/step", post(sim_step))
        .route("/sim/auto", post(sim_auto))
        .route("/sim/persona", post(sim_persona))
        .route("/sim/peer-block", post(sim_peer_block))
        .route("/sim/state", get(sim_state))
        .with_state(handle)
}

/// send one control command and await its reply; a closed actor is a 503
/// (the sim is shutting down), matching how the /v1 lane degrades.
async fn control<T, F>(handle: SimHandle, build: F) -> Result<T, Response>
where
    F: FnOnce(oneshot::Sender<T>) -> SimCommand,
{
    let (reply_tx, reply_rx) = oneshot::channel();
    let mut sender = handle.control;
    if sender.try_send(build(reply_tx)).is_err() {
        return Err(StatusCode::SERVICE_UNAVAILABLE.into_response());
    }
    reply_rx
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE.into_response())
}

async fn sim_step(State(handle): State<SimHandle>) -> Response {
    match control(handle, |reply| SimCommand::Step { reply }).await {
        Ok(report) => Json(report).into_response(),
        Err(resp) => resp,
    }
}

async fn sim_auto(State(handle): State<SimHandle>, Json(req): Json<AutoRequest>) -> Response {
    match control(handle, |reply| SimCommand::SetAuto {
        enabled: req.enabled,
        reply,
    })
    .await
    {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(resp) => resp,
    }
}

async fn sim_persona(State(handle): State<SimHandle>, Json(req): Json<PersonaRequest>) -> Response {
    match control(handle, |reply| SimCommand::SetPersona {
        persona: req.persona,
        reply,
    })
    .await
    {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(resp) => resp,
    }
}

async fn sim_peer_block(
    State(handle): State<SimHandle>,
    Json(req): Json<PeerBlockRequest>,
) -> Response {
    let payload = match serde_json::to_vec(&req.payload) {
        Ok(bytes) => bytes,
        Err(err) => return (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    };
    let origin = req
        .origin
        .map(String::into_bytes)
        .unwrap_or_else(|| PEER_ORIGIN.to_vec());
    match control(handle, |reply| SimCommand::PeerBlock {
        target: req.target,
        payload,
        origin,
        reply,
    })
    .await
    {
        Ok(Ok(info)) => Json(info).into_response(),
        Ok(Err(rejection)) => (StatusCode::BAD_REQUEST, rejection).into_response(),
        Err(resp) => resp,
    }
}

async fn sim_state(State(handle): State<SimHandle>) -> Response {
    match control(handle, |reply| SimCommand::Snapshot { reply }).await {
        Ok(snapshot) => Json(snapshot).into_response(),
        Err(resp) => resp,
    }
}

// ── Networked-persona receipt shaping ───────────────────

/// the networked validator's submit reply is height-only — `opHash` is a
/// local-daemon receipt field. noded's shared submit handler always adds it,
/// so the networked persona strips it at the response layer instead of
/// forking the handler.
async fn strip_receipt_op_hash(
    State(persona): State<Arc<Mutex<Persona>>>,
    req: Request,
    next: Next,
) -> Response {
    let is_submit = req.uri().path() == "/v1/submit";
    let resp = next.run(req).await;
    if !is_submit
        || resp.status() != StatusCode::OK
        || *persona.lock().expect("persona poisoned") != Persona::Networked
    {
        return resp;
    }
    let (mut parts, body) = resp.into_parts();
    let bytes = match axum::body::to_bytes(body, RECEIPT_BODY_CAP).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let stripped = serde_json::from_slice::<serde_json::Value>(&bytes)
        .ok()
        .and_then(|mut value| {
            value.as_object_mut()?.remove("opHash");
            serde_json::to_vec(&value).ok()
        });
    // the buffered body replaces the streamed one, so the recorded length is
    // stale either way — drop it and let hyper size the new body.
    parts.headers.remove(header::CONTENT_LENGTH);
    match stripped {
        Some(new_body) => Response::from_parts(parts, Body::from(new_body)),
        // not a json object (unexpected) — pass the original bytes through.
        None => Response::from_parts(parts, Body::from(bytes)),
    }
}
