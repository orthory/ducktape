//! The Agents register as a module-owned view: who may act, what they may
//! do, and under whose grant, drawn from the registry rows the desktop app
//! pushes. Nothing leaves this view — the register is read-only here.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    AgentsView,
    "Agents",
    "The registry of who may act, what they may do, and under whose grant.",
    ["agents"]
);
