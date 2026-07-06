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
//! with it, qmdb modules, the forge repo, and the per-module index persist;
//! the local block counter resumes ABOVE the index's watermark, so op-log
//! heights stay monotonic across restarts (a counter restarting at 0 would
//! make every new block look already-indexed and be silently skipped).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent::AgentModule;
use runs::RunsModule;
use automations::Automations;
use chat::Chat;
use commonware_runtime::telemetry::metrics::{EncodeLabelSet, MetricsExt as _, Registered, raw};
use commonware_runtime::{Metrics as _, Runner as _, Supervisor as _};
use dispatch::DispatchModule;
use tagging::TaggingModule;
use dispatch_oracle::DispatchWorker;
use document::Document;
use files::Files;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, DispatchRecord, Host, SubmitError};
use inbox::Inbox;
use indexer::IndexStore;
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
use tasks::Tasks;
use tokio::sync::broadcast;

/// every module registered at genesis, in registry order. status reports use
/// this list; keep it in sync with the genesis vec in `run_node`.
const MODULE_IDS: [&str; 16] = [
    "chat",
    "saga",
    "dispatch",
    "tagging",
    "tasks",
    "inbox",
    "automations",
    "jobs",
    "agent",
    "runs",
    "document",
    "pages",
    "forge",
    "files",
    "memory",
    "profiles",
];
const ORACLE_ORIGIN: &[u8] = b"oracle";

/// histogram buckets for block apply latency, in SECONDS (Prometheus
/// convention). ~100µs to ~1s — the range one local block apply falls in.
const LATENCY_BUCKETS: [f64; 13] = [
    0.0001, 0.00025, 0.0005, 0.001, 0.0025, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0,
];

/// labels for the per-dispatch counter. kept LOW-CARDINALITY: `module` is the
/// bounded registered set; `origin` is the trigger KIND only — never the
/// specific submitter name or emitter id.
#[derive(Clone, Debug, Hash, PartialEq, Eq, EncodeLabelSet)]
struct DispatchLabels {
    module: String,
    origin: String,
}

/// the daemon's own Prometheus series, registered INTO commonware's runtime
/// registry so one `context.encode()` (GET /metrics) serves runtime + app
/// metrics together. each `Registered` handle is retained for the process life;
/// updates go through its `Deref` to the underlying metric.
struct NodeMetrics {
    block_height: Registered<raw::Gauge>,
    blocks_total: Registered<raw::Counter>,
    apply_latency: Registered<raw::Histogram>,
    dispatch_total: Registered<raw::Family<DispatchLabels, raw::Counter>>,
}

impl NodeMetrics {
    /// register the `ducktape_*` series on the runtime context (root context, so
    /// names carry no child prefix).
    fn register<C: commonware_runtime::Metrics>(context: &C) -> Self {
        Self {
            block_height: context.gauge(
                "ducktape_block_height",
                "latest committed local block height",
            ),
            blocks_total: context.counter(
                "ducktape_blocks_total",
                "committed local blocks since daemon start",
            ),
            apply_latency: context.histogram(
                "ducktape_block_apply_latency_seconds",
                "node-local wall-clock cost of applying one block",
                LATENCY_BUCKETS.into_iter(),
            ),
            dispatch_total: context.family(
                "ducktape_dispatch_total",
                "module dispatches, by module and trigger-origin kind",
            ),
        }
    }

    /// fold one committed block's telemetry into the series.
    fn record_block(&self, height: u64, latency_us: u64, dispatches: &[DispatchRecord]) {
        self.block_height.set(height as i64);
        self.blocks_total.inc();
        // microseconds → seconds for the Prometheus convention.
        self.apply_latency.observe(latency_us as f64 / 1_000_000.0);
        for d in dispatches {
            self.dispatch_total
                .get_or_create(&DispatchLabels {
                    module: d.module.clone(),
                    origin: origin_kind(&d.origin).to_string(),
                })
                .inc();
        }
    }
}

/// the low-cardinality trigger KIND of a dispatch origin — the metrics label
/// (`origin_tag` below keeps the specific name/id for the telemetry frame).
fn origin_kind(origin: &Origin) -> &'static str {
    match origin {
        Origin::External(_) => "external",
        Origin::Module(_) => "module",
        Origin::System => "system",
    }
}

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

    // the per-module derived index: one fluent31 database per module under
    // <storage>/index/<module>/, with each module's view mapper registered.
    // an open failure is fatal-with-remedy rather than a silent no-index run:
    // the tier is rebuildable, so the fix is always "delete <storage>/index".
    let index = noded::open_index_store(&storage, &MODULE_IDS)?;

    let (handle, cmd_rx, event_tx) = NodeHandle::channel();
    let handle = handle
        .with_forge_repo(forge_repo.clone())
        .with_index_store(index.clone());

    // the node actor gets its own thread: commonware's tokio runner owns that
    // thread's runtime, and the host must never leave it. the blob handle and
    // the telemetry ring are the node-local surfaces that cross: the actor
    // registers the files module over the blobs and pushes per-block telemetry
    // into the ring, while the http layer reads both through its own clones.
    let actor_storage = storage.clone();
    let actor_forge_repo = forge_repo.clone();
    let actor_index = index.clone();
    let blobs = handle.blob_handle();
    let telemetry = handle.telemetry_ring();
    std::thread::Builder::new()
        .name("node-actor".into())
        .spawn(move || {
            run_node(
                actor_storage,
                actor_forge_repo,
                actor_index,
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
    index: Arc<IndexStore>,
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
        let chat = Chat::init(context.child("chat"), "chat")
            .await
            .with_tagging("tagging");
        let saga = SagaModule::new("saga");
        // the task plane: recipe manifests + capability dispatch with
        // next-block result delivery.
        let dispatch = DispatchModule::new("dispatch", "saga");
        // the engagement plane: tag reports in, engagement events out.
        let tagging = TaggingModule::new("tagging");
        let tasks = Tasks::new("tasks");
        let inbox = Inbox::new("inbox");
        let automations = Automations::new("automations", "chat", "tasks", "inbox", "memory");
        let jobs = Jobs::new("jobs");
        let agent = AgentModule::new("agent", "saga", Some("runs".into()));
        let runs = RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            Some("jobs".into()),
            Some("document".into()),
        );
        let document = Document::init(context.child("document"), "document").await;
        let pages = Pages::init(context.child("pages"), "pages").await;
        // forge shares the files body plane so a Push's packfile — uploaded to
        // the blob lane before the op is submitted — materializes locally; the
        // pack bytes never enter consensus (root stays sha256(head oid)).
        let forge = Forge::with_blobs("forge", forge_repo, blobs.clone()).expect("forge init");
        // the block loop's own handle: each block's root payload is staged as
        // its explorer row is built, so op hashes stay dereferencable via the
        // blob lane (worker follow-ups included — the http submit handler only
        // stages what clients POST).
        let op_blobs = blobs.clone();
        let files = Files::with_blobs("files", blobs);
        let memory = Memory::new("memory", "files");
        // the origin-gated display-name registry: maps each verified submit
        // origin to a chosen name so the ui can resolve authors to names.
        let profiles = Profiles::new("profiles");
        let mut host = Host::genesis(vec![
            Box::new(chat),
            Box::new(saga),
            Box::new(dispatch),
            Box::new(tagging),
            Box::new(tasks),
            Box::new(inbox),
            Box::new(automations),
            Box::new(jobs),
            Box::new(agent),
            Box::new(runs),
            Box::new(document),
            Box::new(pages),
            Box::new(forge),
            Box::new(files),
            Box::new(memory),
            Box::new(profiles),
        ])
        .expect("genesis");

        println!("[noded] genesis app_hash={}", hex_root(&host.app_hash()));

        // register the daemon's `ducktape_*` series on the runtime registry —
        // one `context.encode()` then serves them alongside commonware's own
        // runtime metrics. the handles are retained for the block loop's life.
        let metrics = NodeMetrics::register(&context);

        let workers = oracle_workers();
        // resume the local block counter ABOVE the index watermark: the op
        // log persists under --storage, and a counter restarting at 0 would
        // re-use indexed heights — every new block silently skipped.
        let mut height = index.resume_height().expect("read index watermarks");
        if height > 0 {
            println!("[noded] module index resumes at height {height}");
        }
        // heal modules whose watermark trails the resume floor — a wiped (or
        // torn) per-module database that forward folding can never refill,
        // because its heights are already spent above it. views re-derive
        // from local canonical state at the floor; module-level op history
        // below it starts over at the boundary, visibly via /v1/index/status.
        match noded::rebuild_stale_modules(
            &index,
            &host,
            indexer::RebuildMeta { height, time: 0 },
        )
        .await
        {
            Ok(rebuilt) => {
                for (module, rows) in rebuilt {
                    println!(
                        "[noded] module index for {module} re-derived from state at height {height} ({rows} rows)"
                    );
                }
            }
            Err(err) => {
                // poisoned, not fatal: reads stay up, writes refuse, and the
                // wipe-to-rebuild remedy is the same one the fold error names.
                eprintln!(
                    "[noded] module index rebuild failed: {err} — wipe {} to rebuild",
                    index.base().display()
                );
            }
        }
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
                        &index,
                        &op_blobs,
                        &events,
                        &telemetry,
                        &metrics,
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
                            category: ModuleCategory::of(id),
                        })
                        .collect();
                    let _ = reply.send(NodeStatus {
                        version: env!("CARGO_PKG_VERSION").into(),
                        app_hash: hex_root(&host.app_hash()),
                        height,
                        modules,
                        // the embedded daemon has no mesh identity — clients
                        // treat an empty key as "no peer-routed features here".
                        public_key: String::new(),
                    });
                }
                NodeCommand::Metrics { reply } => {
                    // the context owns the registry; encode it to OpenMetrics text.
                    let _ = reply.send(context.encode());
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

fn oracle_workers() -> Vec<Box<dyn reactor::Worker>> {
    #[cfg(debug_assertions)]
    {
        if std::env::var_os("DUCKTAPE_NODED_ECHO_ORACLE").is_some() {
            return vec![Box::new(EchoWorker)];
        }
    }
    vec![Box::new(DispatchWorker::new(
        // BYO: run whatever executor CLIs the capability specs describe and
        // this host has installed — no credential handling here (see
        // docs/capability-spec.md). a broken operator spec is a boot error.
        capability_host::discover()
            .unwrap_or_else(|e| panic!("capability specs failed to load: {e}")),
        // the daemon's oracle identity: its worker follow-ups are
        // submitted under ORACLE_ORIGIN, so an Accept claim records that
        // key as the assignee and the re-emitted request must match it.
        ORACLE_ORIGIN.to_vec(),
    ))]
}

/// commit the caller's op, then drain worker follow-ups (each its own block).
/// the returned summary is the block that INCLUDED the caller's op — follow-up
/// blocks reach clients over the ws stream, not this reply.
async fn submit_and_drain(
    host: &mut Host,
    workers: &[Box<dyn reactor::Worker>],
    height: &mut u64,
    index: &IndexStore,
    blobs: &files::BlobHandle,
    events: &broadcast::Sender<WsFrame>,
    telemetry: &TelemetryRing,
    metrics: &NodeMetrics,
    origin: Origin,
    msg: Msg,
) -> Result<BlockSummary, String> {
    let (included, effects) = match submit_one(
        host, height, index, blobs, events, telemetry, metrics, origin, msg,
    )
    .await
    {
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

    loop {
        let Some(follow) = queue.pop_front() else {
            // the never-pop-stack tail: results committed into the dispatch
            // mailbox deliver in a LATER block, and this block-per-op daemon
            // ticks no other blocks — nudge one flush block per pending batch.
            if !host.has_pending_deliveries().await {
                break;
            }
            queue.push_back(Msg {
                target: dispatch::DEFAULT_DISPATCH_TARGET.into(),
                payload: dispatch::encode_msg(&dispatch::DispatchMsg::Nudge {}),
            });
            continue;
        };
        rounds += 1;
        if rounds > MAX_WORKER_ROUNDS {
            return Err("worker-round budget exceeded".into());
        }
        match submit_one(
            host,
            height,
            index,
            blobs,
            events,
            telemetry,
            metrics,
            Origin::External(ORACLE_ORIGIN.to_vec()),
            follow,
        )
        .await
        {
            Ok((_block, effects)) => {
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

    Ok(included)
}

async fn submit_one(
    host: &mut Host,
    height: &mut u64,
    index: &IndexStore,
    blobs: &files::BlobHandle,
    events: &broadcast::Sender<WsFrame>,
    telemetry: &TelemetryRing,
    metrics: &NodeMetrics,
    origin: Origin,
    msg: Msg,
) -> Result<(BlockSummary, Vec<Effect>), SubmitError> {
    let consensus_time = unix_millis();
    // the explorer row's identity: capture the root op's coordinates before
    // ctx/msg consume them. this lane frames and signs nothing, so the
    // "proposer" is the SUBMITTER's origin bytes, hex like the networked
    // lane's keys (the profiles registry is keyed the same way, so the
    // console resolves it to a display name).
    let proposer = match &origin {
        Origin::External(id) => hex_bytes(id),
        Origin::Module(id) => format!("module:{id}"),
        Origin::System => "system".into(),
    };
    let target = msg.target.clone();
    let payload = payload_preview(&msg.payload);
    // staging IS hashing: put_chunk keys the blob by sha256, so this both
    // computes the op's content address and keeps it dereferencable via
    // GET /v1/files/blob/{op_hash} — receipt-parity with the submit reply,
    // and coverage for worker follow-ups no client ever POSTed.
    let op_hash = hex_bytes(&blobs.put_chunk(msg.payload.clone()));
    let ctx = BlockContext {
        protocol_version: 0,
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
    let operations: Vec<DispatchInfo> = out.dispatches.iter().map(dispatch_info).collect();
    let frame = TelemetryFrame {
        height: *height,
        consensus_time,
        latency_us,
        dispatches: operations.clone(),
        events: out.events.iter().map(event_preview).collect(),
    };
    // fold this block into the Prometheus series (before `out` is consumed).
    metrics.record_block(*height, latency_us, &out.dispatches);

    // buffer for GET /v1/telemetry, then fan both frames out live. no subscribers
    // is fine — send only fails then.
    telemetry.push(frame.clone());
    let _ = events.send(WsFrame::Block(block.clone()));
    let _ = events.send(WsFrame::Telemetry(frame));

    // fold the block into the derived per-module index LAST: canonical state
    // is already committed, so an index failure degrades the read models and
    // never the block. the store poisons itself on error (contiguity over
    // coverage) and stays loud on every later block until rebuilt.
    let block_ops = indexer::BlockOps {
        // the explorer row. every block on this lane is applied — a rejected
        // submit never increments the height, so it never was a block. the
        // frame hash stays empty: nothing is framed on this lane, and an
        // invented digest would claim a verification that never happened.
        record: Some(block_row(&BlockRecord {
            height: *height,
            hash: String::new(),
            commit_hash: hex_root(&out.app_hash),
            proposer,
            disposition: BlockDisposition::Applied,
            target,
            operations,
            payload,
            op_hash,
        })),
        ..noded::index_block_ops(*height, consensus_time, &out.dispatches)
    };
    if let Err(err) = index.apply_block(&block_ops) {
        eprintln!(
            "[noded] module index apply failed at height {}: {err} — wipe <storage>/index to rebuild",
            *height
        );
    }

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
                Ok(reactor::WorkOutcome::Handled(follow)) => {
                    queue.extend(follow);
                    claimed = true;
                    break;
                }
                Ok(reactor::WorkOutcome::NotMine) => {}
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
    async fn run(&self, effect: &Effect) -> Result<reactor::WorkOutcome, reactor::Error> {
        let request = match saga::decode_worker_request(&effect.0) {
            Ok(request) => request,
            Err(_) => return Ok(reactor::WorkOutcome::NotMine),
        };
        // a dispatch-plane WorkSpec echoes its raw-text lane (the dispatch
        // module judged a Text contract; the agent module normalizes).
        let Ok(work) = dispatch::decode_work_spec(&request.spec) else {
            return Ok(reactor::WorkOutcome::NotMine);
        };
        Ok(reactor::WorkOutcome::Handled(Some(Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&saga::SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome: Ok(format!("echo: handling dispatch {}", work.dispatch_id).into_bytes()),
            }),
        })))
    }
}
