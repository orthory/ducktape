//! the embedded node actor: owns the (non-Send) host for the app's lifetime.
//!
//! same actor the gateway runs behind axum, embedded: genesis the module set,
//! drain commands in arrival order — one submit = one block — and report each
//! committed block through the injected `on_block` (which the shell turns into
//! webview events). module state persists under the app-data storage root the
//! shell passes in; the height counter restarts at 0 each launch — it is a
//! local block counter, not consensus state.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use agent::Agent;
use chat::Chat;
use commonware_runtime::{Runner as _, Supervisor as _};
use document::Document;
use forge::Forge;
use futures::StreamExt as _;
use futures::channel::{mpsc, oneshot};
use host::{BlockContext, Host};
use sdk::{Msg, Origin, StateRoot};
use tasks::Tasks;

/// inbound command backlog before submit/query callers see backpressure.
pub const COMMAND_BUFFER: usize = 64;

/// every module registered at genesis, in registry order. status reports use
/// this list; keep it in sync with the genesis vec in `run`.
const MODULE_IDS: [&str; 5] = ["chat", "tasks", "document", "agent", "forge"];

/// one finalized block, as reported to the webview (command reply + event).
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockSummary {
    pub height: u64,
    pub app_hash: String,
}

/// the status projection: global app-hash plus each registered module's root.
#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeStatus {
    pub app_hash: String,
    pub height: u64,
    pub modules: Vec<ModuleStatus>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub id: String,
    pub root: String,
}

/// a request to the actor. replies cross the channel wire-ready.
pub enum NodeCommand {
    Submit {
        target: String,
        payload: Vec<u8>,
        reply: oneshot::Sender<Result<BlockSummary, String>>,
    },
    Query {
        target: String,
        req: Vec<u8>,
        reply: oneshot::Sender<Result<Vec<u8>, String>>,
    },
    Status {
        reply: oneshot::Sender<NodeStatus>,
    },
}

/// hex-encode a state root for the wire.
fn hex_root(root: &StateRoot) -> String {
    root.0.iter().map(|b| format!("{b:02x}")).collect()
}

/// wall-clock millis for `consensus_time`: a single-writer local node has no
/// consensus clock, and wall time keeps module timestamps meaningful to the ui.
fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is past the epoch")
        .as_millis() as u64
}

/// own the host on this thread until the command channel closes.
pub fn run(
    storage: PathBuf,
    mut cmds: mpsc::Receiver<NodeCommand>,
    on_block: impl Fn(BlockSummary) + Send + 'static,
) {
    let forge_repo = storage.join("forge-git");
    let rt_cfg = commonware_runtime::tokio::Config::default().with_storage_directory(storage);
    let executor = commonware_runtime::tokio::Runner::new(rt_cfg);

    executor.start(|context| async move {
        let chat = Chat::init(context.child("chat"), "chat").await;
        let tasks = Tasks::new("tasks");
        let document = Document::init(context.child("document"), "document").await;
        let agent =
            Agent::init_with_messaging_id(context.child("agent"), "agent", "agent-messaging")
                .await;
        let forge = Forge::init("forge", forge_repo).expect("forge init");
        let mut host = Host::genesis(vec![
            Box::new(chat),
            Box::new(tasks),
            Box::new(document),
            Box::new(agent),
            Box::new(forge),
        ])
        .expect("genesis");

        let mut height = 0u64;
        while let Some(cmd) = cmds.next().await {
            match cmd {
                NodeCommand::Submit {
                    target,
                    payload,
                    reply,
                } => {
                    let ctx = BlockContext {
                        height: height + 1,
                        consensus_time: unix_millis(),
                        origin: Origin::External(b"desktop".to_vec()),
                    };
                    let outcome = host.submit_at(ctx, Msg { target, payload }).await;
                    let result = match outcome {
                        Ok(out) => {
                            height += 1;
                            let block = BlockSummary {
                                height,
                                app_hash: hex_root(&out.app_hash),
                            };
                            on_block(block.clone());
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
                        app_hash: hex_root(&host.app_hash()),
                        height,
                        modules,
                    });
                }
            }
        }
    });
}
