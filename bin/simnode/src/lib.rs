//! Embeddable deterministic sim node.
//!
//! [`boot`] composes the same node the `ducktape-simnode` binary serves —
//! noded's full `/v1` router plus the `/sim/*` determinism lane — on
//! caller-owned storage and a caller-chosen listen address (`:0` for
//! ephemeral), running on private background threads so an embedder needs no
//! tokio runtime. It returns a synchronous [`SimHandle`] (`step`, `set_auto`,
//! `peer_block`, `state`, `wait`, `shutdown`) and installs NO process-global
//! side effects unless `SimOpts::install_log` is set. The binary is a thin
//! `main` over this.
//!
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
//! "concurrent writer" for optimistic-projection race scenarios. it takes
//! EITHER the single-op `{target, payload, origin?}` shape OR a multi-op
//! `{ops: [{target, payload, origin?}, …]}` shape: the ops array commits ONE
//! block with N members through the host's `submit_block` batch engine (per-op
//! isolation, one shared app-hash), and the reply carries a per-member
//! applied/rejected verdict so a test can pin the host's abort-all-and-replay
//! member isolation.
//!
//! signed-frame lane: `POST /v1/submit/frame` verifies exactly as the real
//! daemon does. it decodes the raw frame bytes with `node::decode_frame` — the
//! SAME codec every validator uses — and commits under the frame's VERIFIED
//! signer as `Origin::External`, with the same hold/auto semantics as the
//! frameless lane (a frame parks like any submit in hold mode; auto commits +
//! drains). the signer key is self-authenticating: verifying a frame needs no
//! mesh, no dispatch pool, no provisioner, so this lane is honest here in a way
//! `/v1/call/ws` (which needs a peer) is not. this is the one lane where
//! authorship is CRYPTOGRAPHIC rather than the trusted-client string
//! convention.
//!
//! `--node-key <64-hex>` fabricates a mesh identity for consensus-op scenarios
//! (huddle membership names a node key): `status().public_key` serves it back
//! instead of the empty default. no mesh sits behind it — the sim routes no
//! peer traffic; it is a value for state ops to reference. a key that is not
//! 32 bytes of hex fails loud at startup.
//!
//! opt-in governance genesis: `--with-valset <hex-pubkey>[,<hex>...]` (comma-
//! separated, and repeatable) appends the kv/valset/governance/upgrade system
//! modules AFTER the default 16, seeding the validator set with the given
//! genesis ed25519 keys exactly like bin/node. `--invite-binding <string>`
//! (default `"sim"`, meaningful only with `--with-valset`) sets the network
//! binding governance verifies invite tokens against. registering the upgrade
//! module makes the host's once-per-block boundary `Advance` ride every sim
//! block automatically. the DEFAULT genesis is byte-identical to before —
//! these modules exist only under the flag.
//!
//! origin hex escape (sim lanes only): a `/v1/submit` or `/sim/peer-block`
//! origin string prefixed `hex:` (e.g. `hex:ab12…`, any even-length hex)
//! decodes to RAW bytes before becoming `Origin::External` — the only way to
//! author as a real 32-byte ed25519 key (governance ballots key on it, and raw
//! pubkey bytes are not valid UTF-8). malformed hex after the prefix is a hard
//! request reject, never a silent fall-through to the literal string.
//!
//! honesty rules: no synthetic-rejection knob (rejection scenarios must use
//! genuinely rejectable ops, so module semantics stay real), no live LLM
//! worker (an external llm call in a determinism tool is a contradiction; the
//! echo worker behind `--echo-oracle` is the only oracle). storage should be a
//! fresh dir per run (the height resumes above the index watermark like
//! noded's, but reused module state defeats the same-script reproducibility
//! this tool exists for).
//!
//! run: `cargo run -p simnode -- [--listen 127.0.0.1:8845] [--storage <dir>]
//!       [--auto] [--persona local|networked] [--echo-oracle]
//!       [--with-valset <hex>[,<hex>...]] [--invite-binding <string>]
//!       [--node-key <64-hex>]`
//!
//! v1 limit, by design: a rejected SINGLE op never becomes a block here
//! (Host::submit_at aborts it pre-commit; only the ordered validator journals
//! rejected frames as blocks). a rejected MEMBER of a `submit_block` batch is
//! different — the batch block commits with the accepted members and the
//! rejected member is reported (never journaled as its own block).

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
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
use duckdns::DuckDns;
use files::Files;
use forge::Forge;
use gateway::Gateway;
use runs::RunsModule;
use tagging::TaggingModule;
// the opt-in `--with-valset` governance genesis modules (registered only under
// the flag; the default genesis stays byte-identical without them).
use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use futures::select;
use governance::Governance;
use host::worker::MAX_WORKER_ROUNDS;
use host::{BlockContext, DispatchRecord, Host, MemberOutcome, SubmitError};
use identity::Identity;
use inbox::Inbox;
use indexer::{AppliedOp, BlockOps, IndexStore};
use jobs::Jobs;
use kv::Kv;
use noded::{
    BlockDisposition, BlockRecord, BlockSummary, DispatchInfo, ModuleCategory, ModuleStatus,
    NodeCommand, NodeHandle, NodeStatus, StreamHub, block_row, hex_bytes, hex_root,
    payload_preview,
};
use pages::Pages;
use saga::SagaModule;
use sdk::{Event, Module, Msg, Origin};
use serde::{Deserialize, Serialize};
use tasks::Tasks;
use statesync::qmdb::QmdbStore;
use upgrade::Upgrade;
use clients::Clients;
use valset::Valset;

/// the DEFAULT module set registered at genesis, in registry order — noded's
/// exact set, so status/roots and query targets match what the app expects of a
/// daemon. the sim-parity conformance lane pins this list against noded; do not
/// change it without also changing the daemon.
const BASE_MODULE_IDS: [&str; 16] = [
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
    "duckdns",
    "gateway",
];

/// the four system modules the opt-in `--with-valset` genesis appends AFTER the
/// default 16, in registry order: the KV store, the membership registry seeded
/// with the genesis validators, governance (the sole authorized author of
/// valset change), and the upgrade coordinator — whose mere registration makes
/// the host-injected once-per-block boundary `Advance` ride every sim block.
const VALSET_MODULE_IDS: [&str; 5] = ["kv", "valset", "clients", "governance", "upgrade"];
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
#[serde(rename_all = "snake_case")]
pub enum Persona {
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

/// one op inside a `/sim/peer-block` request — the single-op body IS one of
/// these, and the multi-op body is an array of them.
#[derive(Deserialize)]
struct PeerOp {
    target: String,
    payload: serde_json::Value,
    origin: Option<String>,
}

/// a peer op's actor-lane fields: `(target, payload_bytes, origin_bytes)`. the
/// `hex:` origin escape is still unresolved here — the actor resolves it.
type PeerOpWire = (String, Vec<u8>, Vec<u8>);

/// `/sim/peer-block` accepts EITHER shape (additive, sim-only wire). untagged:
/// a body with `ops` is a `Batch` (the only shape that carries it); anything
/// else falls through to the original `Single` op. ops-wins if both are present.
#[derive(Deserialize)]
#[serde(untagged)]
enum PeerBlockRequest {
    Batch { ops: Vec<PeerOp> },
    Single(PeerOp),
}

/// one member's verdict in a `/sim/peer-block` batch reply (input order).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemberInfo {
    target: String,
    /// this member's authored origin, hex or the printable convention — the
    /// same rendering the block row's `proposer` uses.
    proposer: String,
    /// `applied` (mutated state) or `rejected` (isolated, rolled back).
    disposition: BlockDisposition,
    /// the module's verbatim rejection reason; absent for an applied member.
    #[serde(skip_serializing_if = "Option::is_none")]
    rejection: Option<String>,
}

/// the multi-op `/sim/peer-block` reply: ONE committed block carrying N members,
/// each with its own applied/rejected verdict.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BatchInfo {
    height: u64,
    app_hash: String,
    /// one entry per input op, in input order.
    members: Vec<MemberInfo>,
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
    /// N ops committed as ONE block via the host's `submit_block` batch engine.
    /// each tuple is `(target, payload_bytes, origin_bytes)`; the `hex:` origin
    /// escape resolves in the actor, same as the single-op path.
    PeerBatch {
        ops: Vec<PeerOpWire>,
        reply: oneshot::Sender<Result<BatchInfo, String>>,
    },
    Snapshot {
        reply: oneshot::Sender<SimSnapshot>,
    },
}

/// the control lane's axum state: a sender into the actor. persona lives
/// separately (shared with the receipt-strip layer) because the middleware
/// must read it per-response without an actor round-trip. (the embedder-facing
/// [`SimHandle`] is a different, richer type — this one is only the /sim router's
/// state.)
#[derive(Clone)]
struct ControlState {
    control: mpsc::Sender<SimCommand>,
}

// ── Embeddable API ──────────────────────────────────────

/// how to compose the node [`boot`] serves — mirrors the binary's flags so an
/// embedder builds the same node without a command line. `Default` is the
/// binary's default composition EXCEPT `install_log`: the binary sets it, an
/// embedder must opt in (see the field).
pub struct SimOpts {
    /// auto mode: every submit commits immediately and drains its follow-ups
    /// (noded's behavior). `false` = hold mode — submits park until [`SimHandle::step`].
    pub auto: bool,
    /// register the deterministic echo oracle (`--echo-oracle`).
    pub echo_oracle: bool,
    /// opt-in governance genesis: raw 32-byte ed25519 validator pubkeys. empty
    /// => the default 16-module set, byte-identical.
    pub valset_keys: Vec<Vec<u8>>,
    /// the invite namespace governance verifies tokens against — meaningful only
    /// with `valset_keys`. defaults to `b"sim"`.
    pub invite_binding: Vec<u8>,
    /// fabricate a mesh identity `status().public_key` serves: a canonical
    /// lowercase-hex ed25519 pubkey (the binary validates 32 bytes at parse).
    /// `None` => empty (no peer-routed features).
    pub node_key: Option<String>,
    /// the receipt/ring persona — local daemon vs networked validator shapes.
    pub persona: Persona,
    /// install noded's PROCESS-GLOBAL tracing subscriber + panic hook (feeding
    /// the log ring). the binary sets this; an embedder leaves it `false` so
    /// `boot` has no global side effects and repeated boots don't stack hooks.
    pub install_log: bool,
}

impl Default for SimOpts {
    fn default() -> Self {
        Self {
            auto: false,
            echo_oracle: false,
            valset_keys: Vec::new(),
            invite_binding: b"sim".to_vec(),
            node_key: None,
            persona: Persona::Local,
            install_log: false,
        }
    }
}

/// compose and start the sim node on `storage` and `listen` (`:0` for an
/// ephemeral port — [`SimHandle::addr`] reports the real one). the block-
/// producing actor and the `/v1` + `/sim` serve loop run on private background
/// threads, so the caller needs no tokio runtime. returns once the listener is
/// bound (`/v1/status` is live). the only process-global side effect is the
/// tracing subscriber, and only under `opts.install_log`.
pub fn boot(storage: &Path, listen: SocketAddr, opts: SimOpts) -> Result<SimHandle, String> {
    let SimOpts {
        auto,
        echo_oracle,
        valset_keys,
        invite_binding,
        node_key,
        persona,
        install_log,
    } = opts;

    // the status module list and the index tier both extend only under valset
    // keys; the default path stays the exact 16-module set the parity lane pins.
    let module_ids: Vec<&'static str> = if valset_keys.is_empty() {
        BASE_MODULE_IDS.to_vec()
    } else {
        BASE_MODULE_IDS
            .iter()
            .chain(VALSET_MODULE_IDS.iter())
            .copied()
            .collect()
    };
    let storage = storage.to_path_buf();
    let forge_repo = storage.join("forge-git");

    // the durable block index: /v1/blocks and /v1/index/* read it, the sim
    // actor feeds it block-by-block — same wiring as the real daemons.
    let index_dir = storage.join("index");
    let index = IndexStore::open(&index_dir, &module_ids)
        .map(|store| {
            Arc::new(
                store
                    .with_indexer(Box::new(chat::index::ChatIndex::new("chat")))
                    .with_indexer(Box::new(tasks::index::TasksIndex::new("tasks")))
                    .with_indexer(Box::new(pages::index::PagesIndex::new("pages"))),
            )
        })
        .map_err(|err| {
            format!(
                "open module index at {}: {err} (derived tier — delete the directory to rebuild)",
                index_dir.display()
            )
        })?;

    // the log ring is a process-GLOBAL subscriber (and stacks a panic hook per
    // call), so wire it ONLY under `install_log` — the binary does; an embedder
    // running several sims in-process must not. without it the handle still owns
    // its own ring, nothing just feeds the global tracing layer.
    let (handle, cmd_rx, stream_hub) = if install_log {
        let log_ring = noded::LogRing::default();
        noded::log::init(Some(log_ring.clone()));
        NodeHandle::channel_with_log_ring(log_ring)
    } else {
        NodeHandle::channel()
    };
    let handle = handle
        .with_forge_repo(forge_repo.clone())
        .with_index_store(index.clone());

    let (control_tx, control_rx) = mpsc::channel::<SimCommand>(16);
    let persona = Arc::new(Mutex::new(persona));
    let fatal: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let public_key = node_key.unwrap_or_default();

    // the actor: owns the non-Send host on its own commonware runner thread,
    // answering the node command lane + the /sim control lane. it holds a node
    // clone (for FATAL request_shutdown) and the shared fatal flag.
    let actor_storage = storage.clone();
    let actor_persona = persona.clone();
    let actor_node = handle.clone();
    let actor_fatal = fatal.clone();
    let blobs = handle.blob_handle();
    let actor = std::thread::Builder::new()
        .name("sim-actor".into())
        .spawn(move || {
            run_sim(
                actor_storage,
                forge_repo,
                index,
                blobs,
                actor_persona,
                auto,
                echo_oracle,
                valset_keys,
                invite_binding,
                public_key,
                module_ids,
                cmd_rx,
                control_rx,
                stream_hub,
                actor_node,
                actor_fatal,
            )
        })
        .map_err(|err| format!("spawn sim-actor thread: {err}"))?;

    // the serve loop on a private 2-worker tokio runtime, so an embedder needs
    // no runtime of its own. it binds, reports the REAL bound addr back over a
    // std channel (so `:0` resolves before boot returns), then serves until
    // shutdown_requested.
    let router_control = control_tx.clone();
    let serve_handle = handle.clone();
    let (addr_tx, addr_rx) = std::sync::mpsc::channel::<Result<SocketAddr, String>>();
    let serve = std::thread::Builder::new()
        .name("sim-serve".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(err) => {
                    let _ = addr_tx.send(Err(format!("build serve runtime: {err}")));
                    return;
                }
            };
            runtime.block_on(async move {
                let listener = match tokio::net::TcpListener::bind(listen).await {
                    Ok(listener) => listener,
                    Err(err) => {
                        let _ = addr_tx.send(Err(format!("bind {listen}: {err}")));
                        return;
                    }
                };
                let bound = match listener.local_addr() {
                    Ok(bound) => bound,
                    Err(err) => {
                        let _ = addr_tx.send(Err(format!("read bound addr: {err}")));
                        return;
                    }
                };
                let shutdown = serve_handle.clone();
                let app = noded::router(serve_handle)
                    .merge(sim_router(ControlState {
                        control: router_control,
                    }))
                    .layer(axum::middleware::from_fn_with_state(
                        persona,
                        strip_receipt_op_hash,
                    ));
                // report the bound addr only once the app is built and about to
                // serve — boot returns after this, so /v1 answers immediately.
                let _ = addr_tx.send(Ok(bound));
                // connect-info so the admin namespace's fail-closed loopback gate
                // sees the (loopback) peer — same as noded::serve.
                let served = axum::serve(
                    listener,
                    app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
                )
                .with_graceful_shutdown(async move { shutdown.shutdown_requested().await })
                .await;
                if let Err(err) = served {
                    tracing::error!(
                        target: "ducktape::node",
                        error = %err,
                        "sim serve loop exited with error"
                    );
                }
            });
        })
        .map_err(|err| format!("spawn sim-serve thread: {err}"))?;

    // block until the serve thread reports its bound addr (or a bind failure). a
    // dropped sender means the thread died before binding. on any error the
    // spawned actor exits on its own: both control senders (this local and the
    // serve thread's) drop, closing its control lane.
    let addr = addr_rx
        .recv()
        .map_err(|_| "serve thread exited before binding".to_string())??;

    Ok(SimHandle {
        addr,
        control: Some(control_tx),
        node: handle,
        serve: Some(serve),
        actor: Some(actor),
        fatal,
    })
}

/// a running sim node, embeddable without a tokio runtime. synchronous: each
/// method sends one control command to the actor and blocks for its reply. the
/// serve loop and actor run on private threads this handle owns — dropping it
/// (or [`Self::shutdown`]) tears both down cleanly. [`Self::wait`] is the
/// binary's path: block until the node stops, surfacing any fatal reason.
pub struct SimHandle {
    addr: SocketAddr,
    /// the /sim control lane into the actor. `Option` so teardown can drop the
    /// last sender (closing the actor's control channel) without moving out of a
    /// `Drop` type. `None` once torn down — calls then return `Err`.
    control: Option<mpsc::Sender<SimCommand>>,
    /// held to request graceful shutdown on teardown (and matched by the actor's
    /// own clone that a FATAL commit fires).
    node: NodeHandle,
    serve: Option<std::thread::JoinHandle<()>>,
    actor: Option<std::thread::JoinHandle<()>>,
    fatal: Arc<Mutex<Option<String>>>,
}

impl SimHandle {
    /// the real bound listen address (`:0` resolved to a concrete port).
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// commit exactly one queued op — an oracle follow-up first, else the oldest
    /// held submit. the [`StepReport`] as JSON, the shape `POST /sim/step` returns.
    pub fn step(&self) -> Result<serde_json::Value, String> {
        let report: StepReport = self.call(|reply| SimCommand::Step { reply })?;
        serde_json::to_value(report).map_err(|err| err.to_string())
    }

    /// toggle auto mode; entering auto flushes the parked backlog.
    pub fn set_auto(&self, enabled: bool) -> Result<(), String> {
        let _: SimSnapshot = self.call(|reply| SimCommand::SetAuto { enabled, reply })?;
        Ok(())
    }

    /// commit a concurrent writer's block past any parked queue. `body` is the
    /// `/sim/peer-block` request shape — a single `{target,payload,origin?}` op
    /// OR a `{ops:[…]}` batch — and the reply matches (`CommittedInfo` or
    /// `BatchInfo`).
    pub fn peer_block(&self, body: serde_json::Value) -> Result<serde_json::Value, String> {
        let request: PeerBlockRequest =
            serde_json::from_value(body).map_err(|err| err.to_string())?;
        match request {
            PeerBlockRequest::Single(op) => {
                let (target, payload, origin) = encode_peer_op(op).map_err(|(_, msg)| msg)?;
                let info = self.call(|reply| SimCommand::PeerBlock {
                    target,
                    payload,
                    origin,
                    reply,
                })??;
                serde_json::to_value(info).map_err(|err| err.to_string())
            }
            PeerBlockRequest::Batch { ops } => {
                let mut encoded = Vec::with_capacity(ops.len());
                for op in ops {
                    encoded.push(encode_peer_op(op).map_err(|(_, msg)| msg)?);
                }
                let info = self.call(|reply| SimCommand::PeerBatch {
                    ops: encoded,
                    reply,
                })??;
                serde_json::to_value(info).map_err(|err| err.to_string())
            }
        }
    }

    /// the current [`SimSnapshot`] as JSON, the shape `GET /sim/state` returns.
    pub fn state(&self) -> Result<serde_json::Value, String> {
        let snapshot: SimSnapshot = self.call(|reply| SimCommand::Snapshot { reply })?;
        serde_json::to_value(snapshot).map_err(|err| err.to_string())
    }

    /// the binary's path: block until the node stops — an external
    /// `/v1/admin/shutdown`, or a FATAL commit that halted it — then join both
    /// threads. `Err(reason)` if a fatal was recorded (`main` turns that into
    /// exit 1). does NOT itself request shutdown, so a normal binary serves
    /// until killed.
    pub fn wait(mut self) -> Result<(), String> {
        self.join_threads();
        match self.fatal.lock().expect("fatal flag poisoned").clone() {
            Some(reason) => Err(reason),
            None => Ok(()),
        }
    }

    /// tear the node down: request graceful shutdown, then join both threads.
    pub fn shutdown(mut self) {
        self.node.request_shutdown();
        self.join_threads();
    }

    /// send one control command and block for its reply. a torn-down actor is
    /// `Err("sim actor stopped")` — the sync twin of the /sim router's 503.
    fn call<T, F>(&self, build: F) -> Result<T, String>
    where
        F: FnOnce(oneshot::Sender<T>) -> SimCommand,
    {
        // fail CLOSED once a fatal commit halted the host. `halt` tears down the
        // serve loop but not the actor (this live handle still holds a control
        // sender), so without this guard a post-fatal step/state/set_auto/
        // peer_block would answer Ok on a corrupt host — every embedded method
        // routes through here, so one check covers all. `wait` surfaces the same
        // reason for the binary path.
        if let Some(reason) = self.fatal.lock().expect("fatal flag poisoned").clone() {
            return Err(reason);
        }
        let mut sender = self
            .control
            .clone()
            .ok_or_else(|| "sim actor stopped".to_string())?;
        let (reply_tx, reply_rx) = oneshot::channel();
        sender
            .try_send(build(reply_tx))
            .map_err(|_| "sim actor stopped".to_string())?;
        futures::executor::block_on(reply_rx).map_err(|_| "sim actor stopped".to_string())
    }

    /// join serve then actor, idempotently. the serve thread ends on
    /// shutdown_requested (dropping its command + control senders); dropping OUR
    /// control sender then closes the actor's control channel so its `select!`
    /// breaks. a test process must exit cleanly, so neither thread is leaked.
    fn join_threads(&mut self) {
        if let Some(serve) = self.serve.take() {
            let _ = serve.join();
        }
        // drop the last /sim control sender so the actor's control lane closes.
        self.control = None;
        if let Some(actor) = self.actor.take() {
            let _ = actor.join();
        }
    }
}

impl Drop for SimHandle {
    fn drop(&mut self) {
        self.node.request_shutdown();
        self.join_threads();
    }
}

// ── The actor ───────────────────────────────────────────

/// a client submit parked until a step commits it: the retained reply is what
/// keeps its http request hanging — the held-submit semantics under test.
struct HeldOp {
    origin: Origin,
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
    workers: Vec<Box<dyn host::worker::Worker>>,
    blobs: blobstore::BlobHandle,
    index: Arc<IndexStore>,
    stream_hub: StreamHub,
    /// the registered module ids, in registry order — the exact set `status`
    /// reports (the default 16, or those plus VALSET_MODULE_IDS under the flag).
    module_ids: Vec<&'static str>,
    /// the fabricated mesh identity `status` reports (`--node-key`), or empty
    /// for the default "no peer-routed features here". no mesh sits behind it.
    public_key: String,
    /// a clone of the node handle, held only so a FATAL commit can request
    /// graceful shutdown (the embeddable replacement for the binary's
    /// `process::exit(1)` — a lib must never kill the host process).
    node: NodeHandle,
    /// set to the fatal reason if a commit hits `SubmitError::Fatal`. the
    /// embedder's [`SimHandle::wait`] surfaces it (the binary turns that into
    /// exit 1); reads/steps then fail because the actor is torn down.
    fatal: Arc<Mutex<Option<String>>>,
}

#[allow(clippy::too_many_arguments)]
fn run_sim(
    storage: PathBuf,
    forge_repo: PathBuf,
    index: Arc<IndexStore>,
    blobs: blobstore::BlobHandle,
    persona: Arc<Mutex<Persona>>,
    auto: bool,
    echo_oracle: bool,
    valset_keys: Vec<Vec<u8>>,
    invite_binding: Vec<u8>,
    public_key: String,
    module_ids: Vec<&'static str>,
    mut cmds: mpsc::Receiver<NodeCommand>,
    mut control: mpsc::Receiver<SimCommand>,
    stream_hub: StreamHub,
    node: NodeHandle,
    fatal: Arc<Mutex<Option<String>>>,
) {
    let duckfs_dir = storage.join("duckfs");
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: noded's exact module set (keep in sync with BASE_MODULE_IDS)
        // so app queries and status roots behave like a real daemon's.
        let chat = Chat::new("chat", Box::new(QmdbStore::init(context.child("chat"), "chat").await))
            .with_tagging("tagging");
        let saga = SagaModule::new("saga");
        let dispatch = DispatchModule::new("dispatch", "saga");
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
        // head from (W2) — mandatory for envelope composition.
        .with_files_module("files")
        // the pages module the composer renders [[page:<id>]] refs from and
        // the pages effects lane writes to; unwired, both degrade.
        .with_pages_module("pages");
        let pages = Pages::new("pages", Box::new(QmdbStore::init(context.child("pages"), "pages").await))
            .with_tagging("tagging");
        let forge = Forge::with_blobs("forge", forge_repo, blobs.clone())
            .expect("forge init")
            .with_chat("chat");
        let files = Files::open("files", duckfs_dir).expect("duckfs open");
        // the deterministic user->nodes binding registry — no valset, no chain
        // (the simulator has neither), matching noded's daemon wiring. It is
        // also the canonical account display-name registry.
        let identity = Identity::new("identity", None, String::new());
        let duckdns = DuckDns::new("duckdns", "identity", None);
        let gateway = Gateway::new("gateway", "identity", None, "local");
        let mut modules: Vec<Box<dyn Module>> = vec![
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
            Box::new(duckdns),
            Box::new(gateway),
        ];
        // opt-in governance genesis, AFTER the default 16 in registry order:
        // kv, valset (seeded with the given genesis validators exactly like
        // bin/node), governance (the sole authorized author of valset change,
        // bound to the invite namespace), and the upgrade coordinator — whose
        // registration alone makes the host's once-per-block boundary `Advance`
        // ride every sim block. modreg/capability are deliberately left out
        // (UpdateModule proposals stay unwired), and saga's construction is
        // untouched. empty valset_keys => the default set, byte-identical.
        if !valset_keys.is_empty() {
            let kv = Kv::new("kv", Box::new(QmdbStore::init(context.child("kv"), "kv").await));
            let mut valset = Valset::new("valset");
            for key in &valset_keys {
                valset.insert(key.clone());
            }
            // the client ACL, seeded empty — a redeemed role=Client invite
            // records a key here (governance emits ClientsMsg::Grant).
            let clients = Clients::new("clients");
            let governance = Governance::new("governance", "valset", "upgrade", "identity")
                .with_invite_binding(invite_binding)
                .with_clients("clients");
            let upgrade = Upgrade::new("upgrade", "valset");
            modules.push(Box::new(kv));
            modules.push(Box::new(valset));
            modules.push(Box::new(clients));
            modules.push(Box::new(governance));
            modules.push(Box::new(upgrade));
        }
        let host = Host::genesis(modules).expect("genesis");

        // a lib must not write to stdout — this is a once-per-boot lifecycle
        // fact, so it rides tracing (visible on the binary's stderr + ring under
        // `install_log`, silent for an embedder that opted out). no sim harness
        // reads this line off stdout; readiness is `/v1/status`.
        tracing::info!(
            target: "ducktape::consensus",
            app_hash = %hex_root(&host.app_hash()),
            "genesis"
        );

        // resume above the index watermark like noded — with the contractual
        // fresh dir this is 0; on a (discouraged) reused dir it keeps op-log
        // heights monotonic instead of silently skipping every new block.
        let height = index.resume_height().expect("read index watermarks");
        stream_hub.prime(height, hex_root(&host.app_hash()));

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
            index,
            stream_hub,
            module_ids,
            public_key,
            node,
            fatal,
        };

        loop {
            select! {
                cmd = control.next() => match cmd {
                    Some(cmd) => sim.handle_control(cmd).await,
                    None => break,
                },
                cmd = cmds.next() => match cmd {
                    Some(NodeCommand::Submit { target, payload, origin, reply }) => {
                        // the `hex:` origin escape resolves to raw bytes here, so a
                        // client can author as a real ed25519 key; malformed hex is
                        // a hard reject, never a literal-string fall-through.
                        match decode_origin(origin) {
                            Ok(origin) => {
                                sim.handle_submit(
                                    Origin::External(origin),
                                    Msg { target, payload },
                                    reply,
                                )
                                .await;
                            }
                            Err(err) => {
                                let _ = reply.send(Err(err));
                            }
                        }
                    }
                    // the signed-frame lane, FAITHFUL to the real daemon: decode
                    // and verify the frame with the same `node::decode_frame` every
                    // validator uses, then hold/commit under the frame's VERIFIED
                    // signer as origin. the signer key is self-authenticating — no
                    // mesh, dispatch pool, or provisioner is needed to check it — so
                    // this lane is honest here in a way `/v1/call/ws` (which needs a
                    // peer) is not. junk never reaches the host: the http gate
                    // already refused it, and this decode is the second wall.
                    Some(NodeCommand::SubmitFrame { frame, reply }) => {
                        match node::decode_frame(&frame) {
                            Ok((origin, msg)) => {
                                sim.handle_submit(origin, msg, reply).await;
                            }
                            Err(err) => {
                                let _ = reply.send(Err(err.to_string()));
                            }
                        }
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
    /// hold or commit ONE op under an already-resolved `Origin` — the shared
    /// lane behind both `/v1/submit` (a caller string, or the `hex:` escape) and
    /// `/v1/submit/frame` (the frame's cryptographically VERIFIED signer). the
    /// two lanes differ ONLY in how the origin was obtained; hold/auto semantics
    /// are identical past this point.
    async fn handle_submit(
        &mut self,
        origin: Origin,
        msg: Msg,
        reply: oneshot::Sender<Result<BlockSummary, String>>,
    ) {
        if !self.auto {
            self.held.push_back(HeldOp { origin, msg, reply });
            return;
        }
        // auto mode = noded's submit_and_drain: commit the caller's op, then
        // drain its worker follow-ups to completion, each its own block.
        let result = self.commit(origin, msg).await;
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
                // same `hex:` origin escape as /v1/submit — a concurrent writer
                // can also author as a raw ed25519 key; bad hex rejects the block.
                let result = match decode_origin(origin) {
                    Ok(origin) => self
                        .commit(Origin::External(origin), Msg { target, payload })
                        .await
                        .map(|committed| committed_info(&committed, "peer")),
                    Err(err) => Err(err),
                };
                let _ = reply.send(result);
            }
            SimCommand::PeerBatch { ops, reply } => {
                // resolve each member's origin through the SAME `hex:` escape as
                // the single-op lane; one bad origin rejects the whole request
                // (before any state moves — nothing has committed yet).
                let resolved: Result<Vec<(Origin, Msg)>, String> = ops
                    .into_iter()
                    .map(|(target, payload, origin)| {
                        decode_origin(origin)
                            .map(|o| (Origin::External(o), Msg { target, payload }))
                    })
                    .collect();
                let result = match resolved {
                    Ok(ops) => self.commit_batch(ops).await,
                    Err(err) => Err(err),
                };
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
                    tracing::warn!(
                        target: "ducktape::modules",
                        error = %err,
                        "worker follow-up REJECTED — the oracle's result never landed"
                    );
                    None
                }
            };
        }
        let held = self.held.pop_front()?;
        let result = self.commit(held.origin, held.msg).await;
        let info = result
            .as_ref()
            .ok()
            .map(|committed| committed_info(committed, "held"));
        let _ = held.reply.send(result.map(|committed| committed.block));
        info
    }

    /// noded's follow-up budget, for auto mode only: manual steps are already
    /// bounded by the test issuing them one at a time. (in HOLD mode a
    /// committed dispatch mailbox flushes on the next explicit step's block —
    /// that is the deterministic-sim semantic, deliberately not auto-nudged.)
    async fn drain_oracle_budgeted(&mut self) -> Result<(), String> {
        let mut rounds = 1u32;
        loop {
            let Some(follow) = self.oracle_queue.pop_front() else {
                // the never-pop-stack tail: results committed into the
                // dispatch mailbox deliver in a LATER block — auto mode
                // settles fully, so nudge one flush block per pending batch.
                if !self.host.has_pending_deliveries().await {
                    break;
                }
                self.oracle_queue.push_back(Msg {
                    target: dispatch::DEFAULT_DISPATCH_TARGET.into(),
                    payload: dispatch::encode_msg(&dispatch::DispatchMsg::Nudge {}),
                });
                continue;
            };
            rounds += 1;
            if rounds > MAX_WORKER_ROUNDS {
                return Err("worker-round budget exceeded".into());
            }
            if let Err(err) = self
                .commit(Origin::External(ORACLE_ORIGIN.to_vec()), follow)
                .await
            {
                tracing::warn!(
                    target: "ducktape::modules",
                    error = %err,
                    "worker follow-up REJECTED — the oracle's result never landed"
                );
            }
        }
        Ok(())
    }

    /// the one commit point — noded's submit_one under the logical clock:
    /// apply the op at the next height, publish the same ws block frame a real
    /// node publishes, stage the payload (op hash == content address), and feed
    /// the durable block index that serves GET /v1/blocks and /v1/index/*.
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
                // green runs over corrupt state. as a LIB we cannot
                // `process::exit` — record the reason, request graceful
                // shutdown, and return the error so the held submitter (if any)
                // gets it and the actor tears down; `SimHandle::wait` surfaces
                // it (the binary turns that into exit 1).
                tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
                self.halt(err.to_string());
                return Err(err.to_string());
            }
            Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
        };
        self.height += 1;

        let block = BlockSummary {
            height: self.height,
            app_hash: hex_root(&out.app_hash),
        };
        let operations: Vec<DispatchInfo> = out.dispatches.iter().map(dispatch_info).collect();

        // fold the block into the durable index LAST, like noded: canonical
        // state is already committed, so an index failure degrades the read
        // models, never the block. the frame hash stays empty — nothing is
        // framed on this lane, same discipline as the real daemon.
        let ops = out
            .dispatches
            .into_iter()
            .map(|d| AppliedOp {
                // reuse noded's mapping verbatim (single source of truth) so
                // index rows read identically across the real and sim daemons —
                // including the printable-name-or-hex author rendering.
                origin: noded::index_origin(&d.origin),
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
                // the sim commits one op per step/block.
                ops: vec![noded::RootOp {
                    proposer,
                    disposition: BlockDisposition::Applied,
                    target: target.clone(),
                    operations,
                    payload,
                    op_hash: op_hash.clone(),
                }],
            })),
        };
        if let Err(err) = self.index.apply_block(&block_ops) {
            tracing::error!(
                target: "ducktape::consensus",
                height = self.height,
                error = %err,
                "module index apply FAILED — the app's views are now STALE; wipe \
                 <storage>/index to rebuild"
            );
        }

        self.stream_hub
            .publish_block(block.height, block.app_hash.clone());

        offer_effects(
            &self.workers,
            block.height,
            out.events,
            &mut self.oracle_queue,
        )
        .await;
        Ok(Committed {
            block,
            op_hash,
            target,
        })
    }

    /// commit N ops as ONE block through the host's `submit_block` batch engine
    /// — per-op isolation with a SINGLE shared app-hash. the batch twin of
    /// [`Self::commit`]: every member's payload is staged (content-addressed
    /// op_hash), the block index row aggregates ALL members (applied AND
    /// rejected, each with its disposition — the real validator's multi-op row
    /// shape, see `drain_actions::block_actions`), one ws frame is published,
    /// and the reply carries a per-member applied/rejected verdict. an empty
    /// `ops` is a valid empty block: height advances, no members, no row.
    async fn commit_batch(&mut self, ops: Vec<(Origin, Msg)>) -> Result<BatchInfo, String> {
        let consensus_time = SIM_EPOCH_MS + (self.height + 1) * SIM_BLOCK_MS;
        // capture each member's (proposer, target, payload) BEFORE submit_block
        // consumes the ops — the row and reply need them after the engine returns.
        let meta: Vec<(String, String, Vec<u8>)> = ops
            .iter()
            .map(|(origin, msg)| {
                (
                    proposer_hex(origin),
                    msg.target.clone(),
                    msg.payload.clone(),
                )
            })
            .collect();
        let ctx = BlockContext {
            protocol_version: 0,
            height: self.height + 1,
            consensus_time,
            // apply_block ignores ctx.origin — each member carries its own, and
            // the once-per-block System injections are System-authored.
            origin: Origin::System,
        };
        let out = match self.host.submit_block(ctx, ops).await {
            Ok(out) => out,
            Err(SubmitError::Fatal(err)) => {
                // same fail-stop as the single-op lane and the real daemons —
                // but as a lib, halt-and-surface rather than `process::exit`.
                tracing::error!(target: "ducktape::node", error = %err, "FATAL: halting");
                self.halt(err.to_string());
                return Err(err.to_string());
            }
            // a member reject is folded into its MemberOutcome, never here: a
            // whole-batch Rejected is only a boundary-injection failure — surface
            // it verbatim (no block committed).
            Err(err @ SubmitError::Rejected(_)) => return Err(err.to_string()),
        };
        self.height += 1;
        let app_hash = hex_root(&out.app_hash);

        // build the per-member reply verdicts + the block row's RootOps in input
        // order, and the per-module AppliedOp feed from the applied members' (and
        // the System injections') dispatch traces.
        let mut members = Vec::with_capacity(meta.len());
        let mut root_ops = Vec::with_capacity(meta.len());
        let mut applied_ops: Vec<AppliedOp> = Vec::new();
        for ((proposer, target, payload), outcome) in meta.into_iter().zip(out.members.iter()) {
            let (disposition, rejection, dispatches): (
                BlockDisposition,
                Option<String>,
                &[DispatchRecord],
            ) = match outcome {
                MemberOutcome::Applied { dispatches } => {
                    (BlockDisposition::Applied, None, dispatches.as_slice())
                }
                MemberOutcome::Rejected { reason } => {
                    (BlockDisposition::Rejected, Some(reason.clone()), &[])
                }
            };
            // an applied member's dispatches feed the per-module index; a rejected
            // member left no committed writes, so it contributes none.
            for d in dispatches {
                applied_ops.push(AppliedOp {
                    origin: noded::index_origin(&d.origin),
                    module: d.module.clone(),
                    payload: d.payload.clone(),
                });
            }
            let operations: Vec<DispatchInfo> = dispatches.iter().map(dispatch_info).collect();
            // stage every member's payload (idempotent, content-addressed) so the
            // row's op_hash dereferences via /v1/files/blob/{opHash} — the
            // validator stages rejected members too (explorer_root_op).
            let op_hash = hex_bytes(&self.blobs.put_chunk(payload.clone()));
            root_ops.push(noded::RootOp {
                proposer: proposer.clone(),
                disposition,
                target: target.clone(),
                operations,
                payload: payload_preview(&payload),
                op_hash,
            });
            members.push(MemberInfo {
                target,
                proposer,
                disposition,
                rejection,
            });
        }
        // the once-per-block System injections also mutate module state — feed
        // their dispatches to the per-module index like the single-op lane.
        for d in &out.system_dispatches {
            applied_ops.push(AppliedOp {
                origin: noded::index_origin(&d.origin),
                module: d.module.clone(),
                payload: d.payload.clone(),
            });
        }

        let block_ops = BlockOps {
            height: self.height,
            time: consensus_time,
            ops: applied_ops,
            // a truly empty batch writes no explorer row (drain_actions' rule);
            // any member — applied or rejected — gives the block its row.
            record: (!root_ops.is_empty()).then(|| {
                block_row(&BlockRecord {
                    height: self.height,
                    hash: String::new(),
                    commit_hash: app_hash.clone(),
                    ops: root_ops,
                })
            }),
        };
        if let Err(err) = self.index.apply_block(&block_ops) {
            tracing::error!(
                target: "ducktape::consensus",
                height = self.height,
                error = %err,
                "module index apply FAILED — the app's views are now STALE; wipe \
                 <storage>/index to rebuild"
            );
        }

        self.stream_hub.publish_block(self.height, app_hash.clone());
        offer_effects(
            &self.workers,
            self.height,
            out.events,
            &mut self.oracle_queue,
        )
        .await;

        Ok(BatchInfo {
            height: self.height,
            app_hash,
            members,
        })
    }

    fn status(&self) -> NodeStatus {
        let modules = self
            .module_ids
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
            // empty unless `--node-key` fabricated one: clients treat an empty
            // key as "no peer-routed features here" (no huddle voice). the
            // seeded key names an identity for consensus-op scenarios; no mesh
            // routes behind it.
            public_key: self.public_key.clone(),
            operations: noded::OperationalStatus {
                role: noded::NodeRole::Local,
                phase: noded::NodePhase::Serving,
                ..Default::default()
            },
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

    /// record a fatal reason and request graceful shutdown — the embeddable
    /// replacement for `process::exit(1)`. only the FIRST reason sticks (a
    /// half-committed host may fault repeatedly as it tears down). the serve
    /// loop drains on `shutdown_requested`; the actor exits when its control
    /// channel closes (the embedder's teardown drops the last sender).
    fn halt(&self, reason: String) {
        let mut fatal = self.fatal.lock().expect("fatal flag poisoned");
        if fatal.is_none() {
            *fatal = Some(reason);
        }
        self.node.request_shutdown();
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

/// resolve a submit/peer-block origin string's raw bytes: an origin prefixed
/// `hex:` decodes the (any even-length) hex remainder to raw bytes — the only
/// way a JSON-string origin lane can name a real 32-byte ed25519 key. malformed
/// hex is an error, never a silent fall-through to the literal `hex:…` bytes.
/// any other origin passes through verbatim (the trusted-client string convention).
fn decode_origin(origin: Vec<u8>) -> Result<Vec<u8>, String> {
    match origin.strip_prefix(b"hex:") {
        Some(rest) => {
            let hex = std::str::from_utf8(rest)
                .map_err(|_| "hex: origin escape is not valid utf-8".to_string())?;
            duckfs_core::unhex(hex).map_err(|e| format!("hex: origin escape: {e}"))
        }
        None => Ok(origin),
    }
}

fn proposer_hex(origin: &Origin) -> String {
    match origin {
        Origin::External(key) => hex_bytes(key),
        Origin::Module(id) => format!("module:{id}"),
        Origin::System => "system".into(),
    }
}

/// map a deterministic dispatch record to its explorer wire twin, keeping the
/// submitter's readable name (`external:<name>`) like noded's rows do — the
/// shared `DispatchInfo::from` deliberately flattens to plain `external`.
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

/// noded's debug echo oracle, unconditional here: the sim is a dev tool, and a
/// deterministic canned reply is the ONLY oracle that belongs in it.
struct EchoWorker;

#[async_trait::async_trait(?Send)]
impl host::worker::Worker for EchoWorker {
    async fn run(&self, event: &Event) -> Result<host::worker::WorkOutcome, host::worker::Error> {
        let request = match saga::decode_worker_request(&event.payload) {
            Ok(request) => request,
            Err(_) => return Ok(host::worker::WorkOutcome::NotMine),
        };
        // a dispatch-plane WorkSpec echoes its raw-text lane (the dispatch
        // module judged a Text contract; the agent module normalizes).
        let Ok(work) = dispatch::decode_work_spec(&request.spec) else {
            return Ok(host::worker::WorkOutcome::NotMine);
        };
        Ok(host::worker::WorkOutcome::Handled(Some(Msg {
            target: "saga".into(),
            payload: saga::encode_msg(&saga::SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome: Ok(format!("echo: handling dispatch {}", work.dispatch_id).into_bytes()),
                usage: None,
            }),
        })))
    }
}

// ── The /sim control routes ─────────────────────────────

fn sim_router(handle: ControlState) -> Router {
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
async fn control<T, F>(handle: ControlState, build: F) -> Result<T, Response>
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

async fn sim_step(State(handle): State<ControlState>) -> Response {
    match control(handle, |reply| SimCommand::Step { reply }).await {
        Ok(report) => Json(report).into_response(),
        Err(resp) => resp,
    }
}

async fn sim_auto(State(handle): State<ControlState>, Json(req): Json<AutoRequest>) -> Response {
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

async fn sim_persona(State(handle): State<ControlState>, Json(req): Json<PersonaRequest>) -> Response {
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

/// encode one peer op's wire fields for the actor: json payload → bytes, and the
/// origin string → bytes (the `hex:` escape is resolved later, in the actor).
/// the default author is `peer`, the concurrent-writer convention. the error is
/// the raw `(status, message)` (small — the caller turns it into a Response).
fn encode_peer_op(op: PeerOp) -> Result<PeerOpWire, (StatusCode, String)> {
    let payload = serde_json::to_vec(&op.payload)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let origin = op
        .origin
        .map(String::into_bytes)
        .unwrap_or_else(|| PEER_ORIGIN.to_vec());
    Ok((op.target, payload, origin))
}

async fn sim_peer_block(
    State(handle): State<ControlState>,
    Json(req): Json<PeerBlockRequest>,
) -> Response {
    match req {
        // the original single-op path: ONE block, one member (unchanged wire).
        PeerBlockRequest::Single(op) => {
            let (target, payload, origin) = match encode_peer_op(op) {
                Ok(parts) => parts,
                Err((status, msg)) => return (status, msg).into_response(),
            };
            match control(handle, |reply| SimCommand::PeerBlock {
                target,
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
        // the multi-op path: N members committed as ONE block via submit_block.
        PeerBlockRequest::Batch { ops } => {
            let mut encoded = Vec::with_capacity(ops.len());
            for op in ops {
                match encode_peer_op(op) {
                    Ok(parts) => encoded.push(parts),
                    Err((status, msg)) => return (status, msg).into_response(),
                }
            }
            match control(handle, |reply| SimCommand::PeerBatch {
                ops: encoded,
                reply,
            })
            .await
            {
                Ok(Ok(info)) => Json(info).into_response(),
                Ok(Err(rejection)) => (StatusCode::BAD_REQUEST, rejection).into_response(),
                Err(resp) => resp,
            }
        }
    }
}

async fn sim_state(State(handle): State<ControlState>) -> Response {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// once a fatal reason is recorded, the embedded control surface fails
    /// closed — every method routes through `call`, so one guard covers all.
    /// we set the flag DIRECTLY rather than provoke a real `SubmitError::Fatal`
    /// (there is no cheap way to force host corruption); the guard is the unit
    /// under test. `wait`'s path is unchanged and covered by the binary suite.
    #[test]
    fn fatal_flag_fails_the_embedded_control_surface_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let handle = boot(
            dir.path(),
            "127.0.0.1:0".parse().expect("addr"),
            SimOpts::default(),
        )
        .expect("boot");
        assert!(handle.state().is_ok(), "healthy before the flag is set");

        *handle.fatal.lock().expect("fatal") = Some("boom".into());
        assert_eq!(handle.step().unwrap_err(), "boom");
        assert_eq!(handle.state().unwrap_err(), "boom");
        assert_eq!(handle.set_auto(false).unwrap_err(), "boom");
        assert_eq!(
            handle
                .peer_block(serde_json::json!({ "target": "chat", "payload": {} }))
                .unwrap_err(),
            "boom",
        );
    }
}
