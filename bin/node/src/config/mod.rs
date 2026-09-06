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

/// compile-validate every deployment `genesis` carries — the same check a
/// validator runs before arming a live swap (`noded::compose::validate_deployment`:
/// `declared_shape` + `check_realizable` + the mapper's `on_apply`/`query`
/// export) — before a founder or a dev shape writes a single byte. Discovery
/// (`Genesis::compose`) only checks filenames, ids, and the Borsh/mapper
/// framing; this is the first point that actually loads each component and
/// mapper through wasmtime, so a zero-byte or truncated artifact refuses
/// `node init` (or `resolve_dev_shape`) instead of minting an unbootable
/// network.
pub fn validate_founding_set(
    source: &std::path::Path,
    genesis: &workspace_config::Genesis,
) -> Result<(), String> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let scratch = std::env::temp_dir().join(format!(
        "ducktape-init-validate-{}-{nanos}",
        std::process::id()
    ));
    // no module id needs its own database here: `validate_deployment` only
    // ever touches the shared blocks database (compiling bytes), never a
    // per-module one.
    let index = indexer::IndexStore::open_bare(&scratch, &[])
        .map_err(|e| format!("open validation store at {}: {e}", scratch.display()))?;
    let result = genesis.modules.iter().try_for_each(|module| {
        noded::compose::validate_deployment(&module.id, &module.bytes, &index).map_err(|error| {
            format!(
                "{}: {error}",
                workspace_config::component_path(source, &module.id).display()
            )
        })
    });
    drop(index);
    let _ = std::fs::remove_dir_all(&scratch);
    result
}
