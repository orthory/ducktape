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

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chat::Chat;
use commonware_runtime::{Runner as _, Supervisor as _};
use document::Document;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::mpsc;
use noded::{BlockSummary, ModuleStatus, NodeCommand, NodeHandle, NodeStatus, hex_root};
use host::{BlockContext, Host};
use sdk::{Msg, Origin};
use tasks::Tasks;
use tokio::sync::broadcast;

/// every module registered at genesis, in registry order. status reports use
/// this list; keep it in sync with the genesis vec in `run_node`.
const MODULE_IDS: [&str; 4] = ["chat", "tasks", "document", "forge"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen: SocketAddr = "127.0.0.1:8844".parse()?;
    let mut storage: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("--listen needs an addr")?.parse()?,
            "--storage" => storage = args.next().map(PathBuf::from),
            other => {
                return Err(
                    format!("unexpected arg {other:?} (want --listen/--storage)").into(),
                );
            }
        }
    }
    let storage = storage.unwrap_or_else(|| {
        std::env::temp_dir().join(format!("ducktape-noded-{}", std::process::id()))
    });

    let (handle, cmd_rx, event_tx) = NodeHandle::channel();

    // the node actor gets its own thread: commonware's tokio runner owns that
    // thread's runtime, and the host must never leave it.
    let actor_storage = storage.clone();
    std::thread::Builder::new()
        .name("node-actor".into())
        .spawn(move || run_node(actor_storage, cmd_rx, event_tx))?;

    println!("[noded] listening on {listen}, storage {}", storage.display());
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
    mut cmds: mpsc::Receiver<NodeCommand>,
    events: broadcast::Sender<BlockSummary>,
) {
    let forge_repo = storage.join("forge-git");
    let rt_cfg =
        commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        // genesis: chat + tasks as the first product surface, document + agent +
        // forge so the whole app-hash story is visible from the status endpoint.
        let chat = Chat::init(context.child("chat"), "chat").await;
        let tasks = Tasks::new("tasks");
        let document = Document::init(context.child("document"), "document").await;
        let forge = Forge::init("forge", forge_repo).expect("forge init");
        let mut host = Host::genesis(vec![
            Box::new(chat),
            Box::new(tasks),
            Box::new(document),
            Box::new(forge),
        ])
        .expect("genesis");

        println!("[noded] genesis app_hash={}", hex_root(&host.app_hash()));

        let mut height = 0u64;
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    origin,
                    reply,
                } => {
                    let ctx = BlockContext {
                        height: height + 1,
                        consensus_time: unix_millis(),
                        origin: Origin::External(origin),
                    };
                    let outcome = host.submit_at(ctx, Msg { target, payload }).await;
                    let result = match outcome {
                        Ok(out) => {
                            height += 1;
                            let block = BlockSummary {
                                height,
                                app_hash: hex_root(&out.app_hash),
                            };
                            // no subscribers is fine — send only fails then.
                            let _ = events.send(block.clone());
                            Ok(block)
                        }
                        Err(err) => Err(err.to_string()),
                    };
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
