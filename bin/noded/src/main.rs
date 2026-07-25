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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use agent::AgentModule;
use automations::Automations;
use chat::Chat;
use commonware_runtime::{Metrics as _, Runner as _, Supervisor as _};
use dispatch::DispatchModule;
use files::Files;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::mpsc;
use gateway::Gateway;
use host::{BlockContext, Host, SubmitError};
use identity::Identity;
use inbox::Inbox;
use indexer::IndexStore;
use noded::{
    BlockDisposition, BlockRecord, BlockSummary, ModuleCategory, ModuleStatus, NodeCommand,
    NodeHandle, NodeMetrics, NodeStatus, ORACLE_ORIGIN, StreamHub, block_row, hex_root,
};
use pages::Pages;
use runs::RunsModule;
use saga::SagaModule;
use sdk::{Event, Msg, Origin};
use statesync::qmdb::QmdbStore;
use tagging::TaggingModule;
use tasks::Tasks;

/// every module registered at genesis, in registry order — the `sim_base`
/// selection of the single-source [`host::topology`] (identical to simnode's
/// default set). status reports use this list; the genesis vec in `run_node`
/// composes the same ids over native module structs.
const MODULE_IDS: &[&str] = host::topology::SIM_BASE;

mod echo_oracle;

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
    let index = noded::open_index_store(&storage, MODULE_IDS)?;

    let log_ring = noded::LogRing::default();
    noded::log::init(Some(log_ring.clone()), Some(storage.join("daemon.log")));

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
    // a full handle clone for the actor loop's own surfaces (status, blobs,
    // the stream hub). The agent-run provisioner is NOT here any more: it lives
    // in the compute daemon and reaches this node over /v1.
    let actor_handle = handle.clone();

    // the node-local, off-chain interactive terminal-session plane (lives in the
    // daemon like the stream hub — never consensus). This node spawns no pty:
    // the plane is the RINGS and the metadata, and an agent daemon (`ducktape
    // service run agent`) attaches over the ws to own the ptys. With none
    // attached, create returns a clear 503 — a bare spawn is unrepresentable.
    let term_ring = handle.stream_hub().terminals();
    let term_cmd_ring = handle.stream_hub().term_commands();
    let handle = handle.with_terminals(noded::TerminalSessions::new(term_ring, term_cmd_ring));
    std::thread::Builder::new()
        .name("node-actor".into())
        .spawn(move || {
            run_node(
                actor_storage,
                actor_forge_repo,
                actor_index,
                blobs,
                actor_handle,
                cmd_rx,
                stream_hub,
            )
        })?;

    tracing::info!(
        target: "ducktape::node",
        %listen,
        storage = %storage.display(),
        "noded listening"
    );
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?
        .block_on(async {
            let listener = tokio::net::TcpListener::bind(listen).await?;
            noded::serve(listener, handle).await?;
            // in-flight requests drained; blocks commit at the block boundary,
            // so exiting here loses nothing.
            tracing::info!(target: "ducktape::node", "noded shutdown requested; exiting");
            Ok(())
        })
}

/// own the host for the process lifetime: genesis the module set, then apply
/// commands in arrival order — every submit is its own block.
// the actor thread's entry point threads every daemon-owned root/lane in by
// value (storage, forge, index, blobs, the actor handle, the command receiver,
// the event fan-out); bundling them into a struct would only rename the list.
#[allow(clippy::too_many_arguments)]
fn run_node(
    storage: PathBuf,
    forge_repo: PathBuf,
    index: Arc<IndexStore>,
    blobs: noded::blobs::BlobHandle,
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
    // Persistent agent workspaces stay under <storage>; portable run mounts
    // live under agent_runs_root outside it.
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: the full product surface. chat/tasks/inbox as the core loop,
        // automations bridging chat events into chat/tasks/inbox follow-ups,
        // jobs for deferred work, pages + forge for the substrate-backed
        // stores, and files (duckfs) for the content plane.
        let chat = Chat::new(
            "chat",
            Box::new(QmdbStore::init(context.child("chat"), "chat").await),
        )
            .with_tagging("tagging");
        let saga = SagaModule::new("saga");
        // the task plane: recipe manifests + capability dispatch with
        // next-block result delivery.
        let dispatch = DispatchModule::new("dispatch", "saga");
        // the engagement plane: tag reports in, engagement events out.
        let tagging = TaggingModule::new(
            "tagging",
            Box::new(QmdbStore::init(context.child("tagging"), "tagging").await),
        )
        .with_direct_owner("runs");
        let tasks = Tasks::new("tasks");
        let inbox = Inbox::new(
            "inbox",
            Box::new(QmdbStore::init(context.child("inbox"), "inbox").await),
        );
        let automations = Automations::new(
            "automations",
            Box::new(QmdbStore::init(context.child("automations"), "automations").await),
            "chat",
            "tasks",
            "inbox",
        );
        let agent = AgentModule::new(
            "agent",
            Box::new(QmdbStore::init(context.child("agent"), "agent").await),
            "saga",
            Some("runs".into()),
        );
        let runs = RunsModule::new(
            "runs",
            "chat",
            "saga",
            "tagging",
            "dispatch",
            "agent",
            Some("tasks".into()),
            Some("tasks".into()),
        )
        // The portable composer pins its source head from duckfs/files.
        .with_files_module("files")
        // the forge module the composer resolves forge:<repo>:<n> channels
        // against and the PR sink queries; unwired, forge-channel mentions
        // skip at compose.
        .with_sink_forge("forge")
        // the pages module the composer renders [[page:<id>]] refs from and
        // the pages effects lane (pages.comment / pages.set_checked) writes
        // to; unwired, both degrade to breadcrumbs.
        .with_pages_module("pages");
        let pages = Pages::new(
            "pages",
            Box::new(QmdbStore::init(context.child("pages"), "pages").await),
        )
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
        // the canonical account display name. store-backed like chat/pages.
        let identity = Identity::new(
            "identity",
            Box::new(QmdbStore::init(context.child("identity"), "identity").await),
            None,
            String::new(),
        );
        // the MERGED gateway owns both the `.duck` handle plane and the route
        // plane; the single-node daemon carries no valset (ungated) and a
        // dev-only chain id.
        let gateway = Gateway::new(
            "gateway",
            Box::new(QmdbStore::init(context.child("gateway"), "gateway").await),
            "identity",
            None,
            "local",
        );
        let mut host = Host::genesis(vec![
            Box::new(chat),
            Box::new(saga),
            Box::new(dispatch),
            Box::new(tagging),
            Box::new(tasks),
            Box::new(inbox),
            Box::new(automations),
            Box::new(agent),
            Box::new(runs),
            Box::new(pages),
            Box::new(forge),
            Box::new(files),
            Box::new(identity),
            Box::new(gateway),
        ])
        .expect("genesis");

        tracing::info!(
            target: "ducktape::consensus",
            root_hash = %hex_root(&host.root_hash()),
            "noded genesis"
        );

        // register the daemon's `ducktape_*` series on the runtime registry —
        // one `context.encode()` then serves them alongside commonware's own
        // runtime metrics. the handles are retained for the block loop's life.
        let metrics = NodeMetrics::register(&context);
        metrics.set_role_phase(noded::NodeRole::Local, noded::NodePhase::Serving);

        // the observability cell: this single-writer loop is the ONE
        // publisher; the status/peers routes read the cell without crossing
        // the command lane, operations overlay live from the metrics, and
        // /metrics + the ws metrics topic encode the registry through the
        // wired exposition source. no mesh identity here — the peers
        // standing stays role-less, and the sample parses honestly empty.
        let status = node_handle.status_cell();
        status.wire_metrics(&metrics);
        // `Context` has no Clone; a child shares the SAME registry (the
        // label only prefixes new registrations), so its encode() serves
        // the identical exposition.
        let exposition_context = context.child("exposition");
        status.wire_exposition(move || exposition_context.encode());

        // NO in-process compute plane: dispatch work is executed by the
        // standalone compute daemon, which reaches this node over its own /v1
        // surface like any other client. What is left here is the reactor seam
        // itself (plus the debug echo the e2e drives).
        let workers = echo_oracle::workers();
        // resume the local block counter ABOVE the index watermark: the op
        // log persists under --storage, and a counter restarting at 0 would
        // re-use indexed heights — every new block silently skipped.
        let mut height = index.resume_height().expect("read index watermarks");
        stream_hub.prime(height, hex_root(&host.root_hash()));
        if height > 0 {
            tracing::info!(
                target: "ducktape::modules",
                height,
                "noded module index resumed"
            );
        }
        // stamp modules whose watermark trails the resume floor — a wiped (or
        // torn) per-module database that forward folding can never refill,
        // because its heights are already spent above it. its feed and views
        // start over at the boundary, visibly via /v1/index/status; history
        // below it re-enters only by replaying blocks through the feed.
        match noded::stamp_stale_modules(&index, height) {
            Ok(stamped) => {
                for module in stamped {
                    tracing::info!(
                        target: "ducktape::modules",
                        module,
                        height,
                        "noded module index stamped backfilled at the boundary"
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
        // the boot snapshot: resumed (or genesis) state serves immediately —
        // /v1/status answers before the first command, never behind it.
        publish_status(&status, &metrics, &index, &host, height);
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
                    publish_status(&status, &metrics, &index, &host, height);
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
                        // the embedded daemon's single-op lane has no batch
                        // path to release a continuation on — refuse loudly
                        // rather than silently strip it off a signed frame.
                        Ok((_origin, _msg, Some(_cont))) => Err(
                            "continuation envelopes are not supported on the embedded daemon lane"
                                .to_string(),
                        ),
                        Ok((origin, msg, None)) => {
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
                    publish_status(&status, &metrics, &index, &host, height);
                    let _ = reply.send(result);
                }
                NodeCommand::Query { target, req, reply } => {
                    let result = host
                        .query(&target, &req)
                        .await
                        .map_err(|err| err.to_string());
                    let _ = reply.send(result);
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
        match submit_one(host, height, index, blobs, stream_hub, metrics, origin, msg).await {
            Ok(out) => out,
            Err(SubmitError::Fatal(err)) => {
                tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
                std::process::exit(1);
            }
            Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
        };

    // the shared reactor loop settles worker follow-ups through this lane's own
    // 1-op-1-block submit path (each its own block), nudging a stranded dispatch
    // mailbox and bounding a self-retriggering worker.
    let mut lane = OracleLane {
        host: &mut *host,
        height: &mut *height,
        index,
        blobs,
        stream_hub,
        metrics,
    };
    let unclaimed = match host::worker::drive(workers, events, &mut lane).await {
        Ok(unclaimed) => unclaimed,
        Err(host::worker::Error::Fatal(err)) => {
            tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
            std::process::exit(1);
        }
        Err(err) => return Err(err.to_string()),
    };

    // an unclaimed event is a module's ONLY diagnostic channel (a wasm guest
    // cannot log) — unless it decodes as a worker request, which means a saga
    // is stuck Pending.
    let mut notes = noded::log::ModuleNotes::new(*height);
    for eff in &unclaimed {
        notes.unclaimed(eff);
    }
    notes.finish();

    Ok(included)
}

/// the noded submit lane behind the shared reactor [`host::worker::drive`]: each
/// worker follow-up commits as its own block through [`submit_one`] under the
/// oracle origin. a deterministic rejection is logged and skipped (the oracle's
/// result never landed); only a fatal block-boundary fault propagates.
struct OracleLane<'a> {
    host: &'a mut Host,
    height: &'a mut u64,
    index: &'a IndexStore,
    blobs: &'a noded::blobs::BlobHandle,
    stream_hub: &'a StreamHub,
    metrics: &'a NodeMetrics,
}

#[async_trait::async_trait(?Send)]
impl host::worker::Lane for OracleLane<'_> {
    async fn submit(&mut self, follow: Msg) -> Result<Vec<Event>, host::worker::Error> {
        match submit_one(
            self.host,
            self.height,
            self.index,
            self.blobs,
            self.stream_hub,
            self.metrics,
            Origin::External(ORACLE_ORIGIN.to_vec()),
            follow,
        )
        .await
        {
            Ok((_block, events)) => Ok(events),
            Err(SubmitError::Fatal(err)) => Err(host::worker::Error::Fatal(err)),
            Err(err @ SubmitError::Rejected(_)) => {
                tracing::warn!(
                    target: "ducktape::modules",
                    error = %err,
                    "worker follow-up REJECTED — the oracle's result never landed"
                );
                Ok(Vec::new())
            }
        }
    }

    async fn pending(&self) -> bool {
        self.host.has_pending_deliveries().await
    }
}

/// assemble and publish the `/v1/status` snapshot for this single-writer
/// lane: at boot, then after every command that can move state. the storage
/// section rides along so index watermarks stay current with the boundary.
fn publish_status(
    status: &noded::StatusCell,
    metrics: &NodeMetrics,
    index: &IndexStore,
    host: &Host,
    height: u64,
) {
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
    status.publish(NodeStatus {
        version: env!("CARGO_PKG_VERSION").into(),
        root_hash: hex_root(&host.root_hash()),
        height,
        modules,
        // the embedded daemon has no mesh identity — clients treat an empty
        // key as "no peer-routed features here".
        public_key: String::new(),
        operations: metrics.operational_status(),
    });
    // no mesh, no consensus: the standing carries only the height — the
    // peers route parses an honestly empty sample with no roles or epoch.
    status.publish_peers(noded::PeersStanding {
        height,
        ..Default::default()
    });
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
    // the row's coordinates, captured before ctx/msg consume them. this lane
    // frames and signs nothing, so the shared projection renders the SUBMITTER's
    // origin as the row `proposer` (hex like the networked lane's keys — identity
    // maps bound node keys to account display names).
    let target = msg.target.clone();
    let payload = msg.payload.clone();
    let ctx = BlockContext {
        height: *height + 1,
        consensus_time,
        origin: origin.clone(),
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
        root_hash: hex_root(&out.root_hash),
    };
    // fold this block into the Prometheus series (before `out` is consumed).
    metrics.record_block(*height, latency_us, &out.dispatches);
    metrics.record_op_outcomes(1, 0); // this lane is one applied member op per block

    // fold the block into the derived per-module index LAST: canonical state
    // is already committed, so an index failure degrades the read models and
    // never the block. the explorer row via the shared projection seam — RootOp
    // assembly, dispatch trace, payload preview, and op-hash staging (put_chunk
    // keys the blob by sha256, keeping it dereferencable via
    // GET /v1/files/blob/{op_hash}) in ONE shape with the validator lane. every
    // block on this lane is applied (a rejected submit never increments the
    // height, so it never was a block); the frame hash stays empty — nothing is
    // framed here, and an invented digest would claim a verification that never
    // happened.
    let record = Some(block_row(&BlockRecord {
        height: *height,
        hash: String::new(),
        commit_hash: hex_root(&out.root_hash),
        // the embedded daemon lane is 1-op-1-block (one host.submit per block),
        // so the block carries exactly one member op.
        ops: vec![noded::projection::project_root_op(
            blobs,
            &origin,
            &target,
            &payload,
            &out.dispatches,
            BlockDisposition::Applied,
        )],
    }));
    // the shared index-fold epilogue (store poisons itself on error, staying
    // loud on every later block until rebuilt).
    noded::projection::apply_block_to_index(
        index,
        *height,
        consensus_time,
        record,
        &out.dispatches,
    );

    // fan the block out live after the derived index had its chance to
    // materialize rows. no subscribers is fine.
    stream_hub.publish_block(block.height, block.root_hash.clone());

    Ok((block, out.events))
}
