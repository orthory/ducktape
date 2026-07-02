//! the ducktape desktop shell: an embedded node behind three tauri commands.
//!
//! the ownership shape is the gateway's, in-process: `host::Host` is non-Send
//! by design, so ONE actor thread owns it inside a commonware tokio runner
//! (the qmdb modules need its storage context, rooted at the OS app-data dir
//! so state is durable across launches) and webview commands reach it over a
//! runtime-agnostic futures channel. every submit is one block; finalized
//! blocks are pushed to the webview as `ducktape://block` events — the tauri
//! twin of the gateway's websocket stream.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod node_actor;

use futures::SinkExt as _;
use futures::channel::{mpsc, oneshot};
use node_actor::{BlockSummary, NodeCommand, NodeStatus};
use tauri::{Emitter as _, Manager as _, State};

/// the command lane into the node actor, managed as tauri state.
struct NodeChannel(mpsc::Sender<NodeCommand>);

/// forward one command to the actor and await its reply.
async fn dispatch<T>(
    channel: &NodeChannel,
    build: impl FnOnce(oneshot::Sender<T>) -> NodeCommand,
) -> Result<T, String> {
    let (reply, rx) = oneshot::channel();
    let mut cmds = channel.0.clone();
    cmds.send(build(reply))
        .await
        .map_err(|_| "node actor is gone".to_string())?;
    rx.await.map_err(|_| "node actor dropped the reply".to_string())
}

#[tauri::command]
async fn node_submit(
    channel: State<'_, NodeChannel>,
    target: String,
    payload: serde_json::Value,
) -> Result<BlockSummary, String> {
    let payload = serde_json::to_vec(&payload).expect("a decoded json value re-serializes");
    dispatch(&channel, |reply| NodeCommand::Submit {
        target,
        payload,
        reply,
    })
    .await?
}

#[tauri::command]
async fn node_query(
    channel: State<'_, NodeChannel>,
    target: String,
    query: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let req = serde_json::to_vec(&query).expect("a decoded json value re-serializes");
    let bytes = dispatch(&channel, |reply| NodeCommand::Query { target, req, reply }).await??;
    serde_json::from_slice(&bytes).map_err(|_| "module reply was not json".to_string())
}

#[tauri::command]
async fn node_status(channel: State<'_, NodeChannel>) -> Result<NodeStatus, String> {
    dispatch(&channel, |reply| NodeCommand::Status { reply }).await
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let storage = app
                .path()
                .app_data_dir()
                .expect("the platform exposes an app-data dir")
                .join("node");
            let (cmd_tx, cmd_rx) = mpsc::channel(node_actor::COMMAND_BUFFER);
            app.manage(NodeChannel(cmd_tx));

            // finalized blocks flow actor -> webview as window events.
            let emitter = app.handle().clone();
            std::thread::Builder::new()
                .name("node-actor".into())
                .spawn(move || {
                    node_actor::run(storage, cmd_rx, move |block| {
                        // a closed webview just drops the event — not an error
                        let _ = emitter.emit("ducktape://block", block);
                    })
                })?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![node_submit, node_query, node_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
