//! This node's operator screen as a view on the kernel contract: status,
//! standing, peers, the log ring and the code registry, rendered from a
//! wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`node.props`: connected, dark, this
//! seat's admin standing and tier, the app's connection reading, the
//! daemon's workspace directory and the wall clock). The node's own facts,
//! its peers and its code registry are read here through the kernel's
//! `rpc.status` / `rpc.peers` / `rpc.query`, re-read on every `rpc.live` hit
//! for the `block` plane, and the log ring arrives through `rpc.stream` on
//! the node's own `logs` topic — the timeline is this view's state, not the
//! app's. Retuning the running node's tracing filter leaves as one
//! `rpc.admin` POST the kernel signs with the seated key; the clipboard is
//! the one intent left, because it is an OS door.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    NodeView,
    "Node",
    "This node: coherent status, standing, peers, logs and the code registry.",
    ["node"]
);
