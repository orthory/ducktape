//! This node's operator screen as a module-owned view: status, standing,
//! peers, the log ring and the code registry, drawn from the facts the
//! desktop app pushes. Tab changes, the log filter, the modules load and a
//! clipboard copy leave as intents; the live log timeline itself is a host
//! surface the app renders in the Activity tab's slot.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    NodeView,
    "Node",
    "This node: coherent status, standing, peers, logs and the code registry.",
    ["node"]
);
