//! node configuration, in two halves.
//!
//! **Reading and writing a workspace** — the identity file, `network.toml`,
//! `node.toml`, the invite codec, and the join ceremony — lives in the
//! `workspace-config` crate, because two very different programs do it: this
//! CLI and the desktop app. It left this binary when the app's only way to join
//! a network was to spawn `ducktape node join` and diff the workspace registry
//! around the call to find out which directory the child had created.
//!
//! **Resolving those files into a runnable daemon** stays here, in
//! [`resolve`]. Only the thing that boots a node needs to; it is where the
//! `[sandbox]` table becomes a live backend gated on a compute grant, and where
//! the dev-seed shape is folded into the same runnable form.
//!
//! Everything is re-exported flat, so `config::<anything>` resolves exactly as
//! it did when all five files sat in this directory.

mod resolve;

pub use resolve::*;
pub use workspace_config::*;
