//! the ducktape node daemon: ONE embedded host behind http/ws.
//!
//! two runtimes, one thread each way: the node actor owns the (non-Send)
//! `host::Host` inside a commonware `tokio::Runner` — the qmdb-backed modules
//! need its storage context — and drains [`NodeCommand`]s in arrival order, one
//! msg per block. the axum server runs on a plain tokio runtime on the main
//! thread and only ever talks to the actor over the command channel. every app
//! build is a client: the web build dials this directly; the desktop shell
//! spawns it detached (an orphan — it outlives the window) and connects the
//! same way. POST /v1/admin/shutdown is how a client retires it: no pid handshake,
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
use commonware_runtime::{Metrics as _, Runner as _, Supervisor as _};
use dispatch::DispatchModule;
use gateway::Gateway;
use tagging::TaggingModule;
use files::Files;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::mpsc;
use host::{BlockContext, DispatchRecord, Host, SubmitError};
use identity::Identity;
use inbox::Inbox;
use indexer::IndexStore;
use jobs::Jobs;
use noded::{
    BlockDisposition, BlockRecord, BlockSummary, DispatchInfo, ModuleCategory, ModuleStatus,
    NodeCommand, NodeHandle, NodeMetrics, NodeStatus, ORACLE_ORIGIN, StreamHub, block_row,
    hex_bytes, hex_root, payload_preview,
};
use pages::Pages;
use host::worker::MAX_WORKER_ROUNDS;
use saga::SagaModule;
use sdk::{Event, Msg, Origin};
use tasks::Tasks;
use statesync::qmdb::QmdbStore;

/// every module registered at genesis, in registry order. status reports use
/// this list; keep it in sync with the genesis vec in `run_node`.
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
    "runs",
    "pages",
    "forge",
    "files",
    "identity",
    "gateway",
];

mod oracle_pool;

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
    // the forge worktree lane's push rendezvous: agent run pushes dial THIS
    // daemon's own http surface at loopback (a wildcard bind is rewritten to
    // 127.0.0.1), where receive-pack submits the ref move to the actor.
    let forge_push_base = noded::agent_provision::forge_push_base(Some(&listen.to_string()));
    // the same surface, bare (no /forge): the base an agent run's tool plane
    // dials back as DUCKTAPE_NODE.
    let node_http_base = noded::agent_provision::node_http_base(Some(&listen.to_string()));

    // the per-module derived index: one fluent31 database per module under
    // <storage>/index/<module>/, with each module's view mapper registered.
    // an open failure is fatal-with-remedy rather than a silent no-index run:
    // the tier is rebuildable, so the fix is always "delete <storage>/index".
    let index = noded::open_index_store(&storage, &MODULE_IDS)?;

    let log_ring = noded::LogRing::default();
    noded::log::init(Some(log_ring.clone()));

    let (handle, cmd_rx, stream_hub) = NodeHandle::channel_with_log_ring(log_ring);
    let handle = handle
        // persist node-local blobs (op receipts, agent prompt pins) under
        // <storage>/blobstore so a daemon restart keeps serving them.
        .with_blob_root(storage.join("blobstore"))?
        .with_forge_repo(forge_repo.clone())
        .with_index_store(index.clone())
        // the duckfs workspace RPC materializes managed checkouts here (disk
        // state, separate from the module's own `<storage>/duckfs` dir).
        .with_duckfs_workspaces(storage.join("duckfs-workspaces"))
        // the single-writer daemon has no consensus and no on-chain owner, so
        // admin stays loopback-trust (ADR A2/A5); `DUCKTAPE_ADMIN=off` still
        // removes the control surface entirely.
        .with_admin(noded::AdminConfig {
            exposure: noded::AdminExposure::from_env(),
            node_key: None,
            ..Default::default()
        });

    // the node actor gets its own thread: commonware's tokio runner owns that
    // thread's runtime, and the host must never leave it. the blob handle is
    // the node-local surface that crosses: the actor registers the files module
    // over the blobs, while the http layer reads through its own clone.
    let actor_storage = storage.clone();
    let actor_forge_repo = forge_repo.clone();
    let actor_index = index.clone();
    let blobs = handle.blob_handle();
    // the oracle pool's re-entry lane: completed provider runs inject their
    // results as Submit commands, exactly as the http layer does.
    let oracle_cmds = handle.command_sender();
    // a full handle clone for the portable-agent-run provisioner: it drives
    // duckfs checkout/commit over this SAME actor lane (the /v1/fs/workspaces
    // transport). cheap — NodeHandle is a command-lane sender + a few Arcs.
    let actor_handle = handle.clone();

    // the node-local, off-chain interactive terminal-session plane (lives in the
    // daemon like the stream hub — never consensus). Podman-only: interactive
    // spawn refuses the Direct backend, so this is available ONLY when the
    // operator configured a sandbox image (DUCKTAPE_SANDBOX_IMAGE); with none,
    // create returns a clear error rather than a Direct spawn. The identity +
    // agent dirs mirror the oracle pool's (ORACLE_ORIGIN, AgentDirs under
    // <storage>). The manager shares the StreamHub's terminal ring so its pump
    // appends where the ws catch-up reads.
    let term_ring = handle.stream_hub().terminals();
    let term_cmd_ring = handle.stream_hub().term_commands();
    let interactive = noded::term::discover_interactive(
        ORACLE_ORIGIN,
        capability_host::AgentDirs::under(&storage),
        noded::term::backend_from_env(),
    );
    tracing::info!(
        target: "ducktape::term",
        enabled = interactive.is_some(),
        "terminal_plane_ready"
    );
    let handle = handle.with_terminals(noded::TerminalSessions::new(
        interactive,
        capability_host::execution_node_id(ORACLE_ORIGIN),
        storage.join("term-sessions"),
        term_ring,
        term_cmd_ring,
    ));
    std::thread::Builder::new()
        .name("node-actor".into())
        .spawn(move || {
            run_node(
                actor_storage,
                actor_forge_repo,
                forge_push_base,
                node_http_base,
                actor_index,
                blobs,
                oracle_cmds,
                actor_handle,
                cmd_rx,
                stream_hub,
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
// the actor thread's entry point threads every daemon-owned root/lane in by
// value (storage, forge, index, blobs, the oracle re-entry lane, the actor
// handle the provisioner drives, the command receiver, the event fan-out);
// bundling them into a struct would only rename the same list.
#[allow(clippy::too_many_arguments)]
fn run_node(
    storage: PathBuf,
    forge_repo: PathBuf,
    forge_push_base: Option<String>,
    node_http_base: Option<String>,
    index: Arc<IndexStore>,
    blobs: noded::blobs::BlobHandle,
    oracle_cmds: mpsc::Sender<NodeCommand>,
    node_handle: noded::NodeHandle,
    mut cmds: mpsc::Receiver<NodeCommand>,
    stream_hub: StreamHub,
) {
    // forge_repo is derived by the caller (shared with the http upload-pack lane).
    let duckfs_dir = storage.join("duckfs");
    // per-agent host state, rooted OUTSIDE <storage> (D7 isolation floor): the
    // persistent executor workspaces + session files must NOT be descendants of
    // the key/consensus/blob tree, so a `..` from a run's cwd can't reach
    // user.key/node keys/qmdb/blobstore. `DUCKTAPE_AGENT_WORKSPACES` / _SESSIONS
    // override — see capability-host. host-local only, never consensus.
    // non-portable (v2/persistent) agent workspaces stay under <storage>, exactly
    // as today — relocating them would be a live (non-dormant) durability change.
    // D7 relocation applies to the PORTABLE provisioner mount (agent_runs_root).
    let agent_dirs = capability_host::AgentDirs::under(&storage);
    // keys the portable run-root's per-node salt + D7 validation (oracle_workers).
    let storage_for_runs = storage.clone();
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: the full product surface. chat/tasks/inbox as the core loop,
        // automations bridging chat events into chat/tasks/inbox follow-ups,
        // jobs for deferred work, pages + forge for the substrate-backed
        // stores, and files (duckfs) for the content plane.
        let chat = Chat::new("chat", Box::new(QmdbStore::init(context.child("chat"), "chat").await))
            .with_tagging("tagging");
        let saga = SagaModule::new("saga");
        // the task plane: recipe manifests + capability dispatch with
        // next-block result delivery.
        let dispatch = DispatchModule::new("dispatch", "saga");
        // the engagement plane: tag reports in, engagement events out.
        let tagging = TaggingModule::new("tagging").with_direct_owner("runs");
        let tasks = Tasks::new("tasks");
        let inbox = Inbox::new("inbox");
        let automations = Automations::new("automations", "chat", "tasks", "inbox");
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
        )
        // the duckfs/files module the portable (v3) composer pins its source
        // head from (W2). its presence is what selects the v3 composer; unwired,
        // the composer emits the v2 wire.
        .with_files_module("files")
        // the forge module the composer resolves forge:<repo>:<n> channels
        // against and the PR sink queries; unwired, forge-channel mentions
        // skip at compose.
        .with_sink_forge("forge")
        // the pages module the composer renders [[page:<id>]] refs from and
        // the pages effects lane (pages.comment / pages.set_checked) writes
        // to; unwired, both degrade to breadcrumbs.
        .with_pages_module("pages");
        let pages = Pages::new("pages", Box::new(QmdbStore::init(context.child("pages"), "pages").await))
            .with_tagging("tagging");
        // forge shares the files body plane so a Push's packfile — uploaded to
        // the blob lane before the op is submitted — materializes locally; the
        // pack bytes never enter consensus (root stays sha256(head oid)).
        let forge = Forge::with_blobs("forge", forge_repo, blobs.clone())
            .expect("forge init")
            .with_chat("chat");
        // the block loop's own handle: each block's root payload is staged as
        // its explorer row is built, so op hashes stay dereferencable via the
        // blob lane (worker follow-ups included — the http submit handler only
        // stages what clients POST).
        let op_blobs = blobs.clone();
        let files = Files::open("files", duckfs_dir).expect("duckfs open");
        // the deterministic user->nodes binding registry. the single-node
        // daemon carries no valset (ungated binds) and no chain (dev-only,
        // chain-unscoped certs are an acceptable surface here). It also owns
        // the canonical account display name.
        let identity = Identity::new("identity", None, String::new());
        // the MERGED gateway owns both the `.duck` handle plane and the route
        // plane; the single-node daemon carries no valset (ungated) and a
        // dev-only chain id.
        let gateway = Gateway::new("gateway", "identity", None, "local");
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
            Box::new(pages),
            Box::new(forge),
            Box::new(files),
            Box::new(identity),
            Box::new(gateway),
        ])
        .expect("genesis");

        println!("[noded] genesis app_hash={}", hex_root(&host.app_hash()));

        // register the daemon's `ducktape_*` series on the runtime registry —
        // one `context.encode()` then serves them alongside commonware's own
        // runtime metrics. the handles are retained for the block loop's life.
        let metrics = NodeMetrics::register(&context);
        metrics.set_role_phase(noded::NodeRole::Local, noded::NodePhase::Serving);

        // OFF-LOOP execution: the pool gates effects inline but runs the
        // provider CLI on spawned tasks; a completed run re-enters as a
        // Submit command on `oracle_cmds`, so this serial command loop
        // never awaits a provider and Query/Status stay responsive while
        // runs are in flight.
        let workers = oracle_pool::oracle_workers(
            &context,
            oracle_cmds,
            node_handle,
            agent_dirs,
            &storage_for_runs,
            forge_push_base,
            node_http_base,
        );
        // resume the local block counter ABOVE the index watermark: the op
        // log persists under --storage, and a counter restarting at 0 would
        // re-use indexed heights — every new block silently skipped.
        let mut height = index.resume_height().expect("read index watermarks");
        stream_hub.prime(height, hex_root(&host.app_hash()));
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
                tracing::error!(
                    target: "ducktape::consensus",
                    error = %err,
                    index = %index.base().display(),
                    "module index rebuild FAILED — the app's views are STALE; wipe the \
                     index directory to rebuild"
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
                        &stream_hub,
                        &metrics,
                        Origin::External(origin),
                        Msg { target, payload },
                    )
                    .await;
                    let _ = reply.send(result); // caller may have hung up
                }
                NodeCommand::SubmitFrame { frame, reply } => {
                    // the frame lane is FAITHFUL to the real node here, and that
                    // is the whole point of it: the origin is the frame's
                    // VERIFIED signer, never the caller's claim. the frameless
                    // arm above trusts a client string — a convention a local
                    // daemon can afford — and `bin/node` discards that string
                    // outright, so an embedded daemon that also stamped a claimed
                    // origin HERE would let an e2e pass on attribution production
                    // would never produce. it decodes exactly as the validator's
                    // ordered drain does instead.
                    let result = match node::decode_frame(&frame) {
                        Ok((origin, msg)) => {
                            submit_and_drain(
                                &mut host,
                                &workers,
                                &mut height,
                                &index,
                                &op_blobs,
                                &stream_hub,
                                &metrics,
                                origin,
                                msg,
                            )
                            .await
                        }
                        // junk never reaches the store: the http gate already
                        // refused it, and this is the second wall for any
                        // embedder-side producer on the command lane.
                        Err(err) => Err(err.to_string()),
                    };
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host
                        .query(&target, &req)
                        .await
                        .map_err(|err| err.to_string());
                    let _ = reply.send(result);
                }
                NodeCommand::Status { reply } => {
                    metrics.update_storage(
                        0,
                        index.is_poisoned(),
                        MODULE_IDS.iter().map(|id| {
                            ((*id).to_string(), index.applied_height(id).unwrap_or_default())
                        }),
                    );
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
                        operations: metrics.operational_status(),
                    });
                }
                NodeCommand::Metrics { reply } => {
                    metrics.update_storage(
                        0,
                        index.is_poisoned(),
                        MODULE_IDS.iter().map(|id| {
                            ((*id).to_string(), index.applied_height(id).unwrap_or_default())
                        }),
                    );
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

/// commit the caller's op, then drain worker follow-ups (each its own block).
/// the returned summary is the block that INCLUDED the caller's op — follow-up
/// blocks reach clients over the ws stream, not this reply.
#[allow(clippy::too_many_arguments)]
async fn submit_and_drain(
    host: &mut Host,
    workers: &[Box<dyn host::worker::Worker>],
    height: &mut u64,
    index: &IndexStore,
    blobs: &noded::blobs::BlobHandle,
    stream_hub: &StreamHub,
    metrics: &NodeMetrics,
    origin: Origin,
    msg: Msg,
) -> Result<BlockSummary, String> {
    let (included, events) =
        match submit_one(host, height, index, blobs, stream_hub, metrics, origin, msg).await
    {
            Ok(out) => out,
            Err(SubmitError::Fatal(err)) => {
                tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
                std::process::exit(1);
            }
            Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
        };

    let mut queue = VecDeque::new();
    offer_effects(workers, *height, events, &mut queue).await;
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
            stream_hub,
            metrics,
            Origin::External(ORACLE_ORIGIN.to_vec()),
            follow,
        )
        .await
        {
            Ok((_block, events)) => {
                offer_effects(workers, *height, events, &mut queue).await;
            }
            Err(SubmitError::Fatal(err)) => {
                tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
                std::process::exit(1);
            }
            Err(err @ SubmitError::Rejected(_)) => {
                tracing::warn!(
                    target: "ducktape::modules",
                    error = %err,
                    "worker follow-up REJECTED — the oracle's result never landed"
                );
            }
        }
    }

    Ok(included)
}

#[allow(clippy::too_many_arguments)]
async fn submit_one(
    host: &mut Host,
    height: &mut u64,
    index: &IndexStore,
    blobs: &noded::blobs::BlobHandle,
    stream_hub: &StreamHub,
    metrics: &NodeMetrics,
    origin: Origin,
    msg: Msg,
) -> Result<(BlockSummary, Vec<Event>), SubmitError> {
    let consensus_time = unix_millis();
    // the explorer row's identity: capture the root op's coordinates before
    // ctx/msg consume them. this lane frames and signs nothing, so the
    // "proposer" is the SUBMITTER's origin bytes, hex like the networked
    // lane's keys (identity maps bound node keys to account display names).
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
    // node-local wall-clock cost of applying this block — the metrics plane's
    // one non-deterministic signal (the apply-latency histogram). measured HERE
    // in the effectful daemon layer, never inside the deterministic host, so
    // the kernel stays clock-free.
    let started = Instant::now();
    let out = host.submit_at(ctx, msg).await?;
    let latency_us = started.elapsed().as_micros() as u64;
    *height += 1;

    let block = BlockSummary {
        height: *height,
        app_hash: hex_root(&out.app_hash),
    };
    let operations: Vec<DispatchInfo> = out.dispatches.iter().map(dispatch_info).collect();
    // fold this block into the Prometheus series (before `out` is consumed).
    metrics.record_block(*height, latency_us, &out.dispatches);
    metrics.record_op_outcomes(1, 0); // this lane is one applied member op per block

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
            // the embedded daemon lane is 1-op-1-block (one host.submit per
            // block), so the block carries exactly one member op.
            ops: vec![noded::RootOp {
                proposer,
                disposition: BlockDisposition::Applied,
                target,
                operations,
                payload,
                op_hash,
            }],
        })),
        ..noded::index_block_ops(*height, consensus_time, &out.dispatches)
    };
    if let Err(err) = index.apply_block(&block_ops) {
        // consensus stays healthy while the ENTIRE app UI silently stops updating:
        // every module view the app reads is served from this derived index.
        tracing::error!(
            target: "ducktape::consensus",
            height = *height,
            error = %err,
            "module index apply FAILED — the app's views are now STALE; wipe \
             <storage>/index to rebuild"
        );
    }

    // fan the block out live after the derived index had its chance to
    // materialize rows. no subscribers is fine.
    stream_hub.publish_block(block.height, block.app_hash.clone());

    Ok((block, out.events))
}

/// map a deterministic dispatch record to its explorer wire twin (the block
/// record's `operations`).
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

async fn offer_effects(
    workers: &[Box<dyn host::worker::Worker>],
    height: u64,
    events: Vec<Event>,
    queue: &mut VecDeque<Msg>,
) {
    let mut notes = noded::log::ModuleNotes::new(height);
    for eff in events {
        let mut claimed = false;
        for w in workers {
            match w.run(&eff).await {
                Ok(host::worker::WorkOutcome::Handled(follow)) => {
                    queue.extend(follow);
                    claimed = true;
                    break;
                }
                Ok(host::worker::WorkOutcome::NotMine) => {}
                Err(err) => {
                    tracing::warn!(
                        target: "ducktape::modules",
                        height,
                        source = %eff.source,
                        error = %err,
                        "worker failed to handle a module event"
                    );
                    claimed = true;
                    break;
                }
            }
        }
        // an unclaimed event is the module's ONLY diagnostic channel (a wasm
        // guest cannot log) — unless it decodes as a worker request, which means
        // a saga is stuck Pending.
        if !claimed {
            notes.unclaimed(&eff);
        }
    }
    notes.finish();
}
