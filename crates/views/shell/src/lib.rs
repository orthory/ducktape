//! Shell as a module-owned view: one question ("run an agent on this
//! network") and the two surfaces that answer it — a durable task
//! transcript and a terminal session — drawn from the facts the desktop app
//! pushes. Every act leaves as an intent the app runs; the composer, the
//! terminal and the answer Markdown are the host's own surfaces.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    ShellView,
    "Shell",
    "Run an agent on this network: durable tasks and a terminal session.",
    ["shell"]
);
