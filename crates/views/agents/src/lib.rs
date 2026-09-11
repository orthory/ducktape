//! The Agents register as a view on the kernel contract: who may act, which
//! executor they run on, the skills they carry and what their runs did,
//! rendered from a wasm component the desktop app loads from a file.
//!
//! The kernel pushes session facts only (`agents.props`: connected, dark,
//! the signing account, and the run another tab opened for the reader). The
//! register, the run tracker and one run's journal are read here through
//! the kernel's `rpc.query` / `rpc.view`, re-read on every `rpc.live` hit
//! for the `runs` and `identity` planes, and a pause or a save leaves as
//! `op.submit` — the runs message the kernel signs with the seated key. The
//! endpoint, the key and the password never cross: a guest that sees no key
//! cannot leak one.

pub mod host;

ui_lang::include_app!("src/ui/app.ice");

ui_lang_guest::export_app!(
    AgentsView,
    "Agents",
    "The registry of who may act, on which executor, and what their runs did.",
    ["agents"]
);
