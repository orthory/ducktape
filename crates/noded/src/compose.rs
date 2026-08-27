//! the ONE module composer: a topology selection + a code source + a store
//! source → the module set. genesis, restore, and statesync in `bin/node`, and
//! the noded/simnode daemons, all build their hosts here — a module's SHAPE
//! (native vs wasm, map/store/odb, committed queries, genesis config) is read
//! from `topology`, never hand-written per composer.
//!
//! what varies per caller is injected, never branched on here: WHERE a store
//! comes from (`QmdbStore::init` on a fresh/reopened dir, `sync_from` at a
//! verified root) is the [`StoreSource`]; WHERE the component bytes come from
//! (a managed dir, the blob plane, the mesh) is the [`host::CodeSource`]; and
//! whether the native registries seed from [`Bindings`] or reopen as committed
//! is the [`Boot`] mode.

use std::collections::BTreeMap;
use std::path::PathBuf;

use sdk::{MerkleStore, Module, StateRoot};
use topology::{Backing, Code, ModuleSpec, TOPOLOGY};
use wasm_host::WasmModule;

/// a boxed, non-`Send` future (the host and every store are `!Send`).
pub type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// the store source: a module id → its opened (or synced) authenticated store.
/// the caller decides the lifecycle; a refused open is an `Err` naming why.
pub type StoreSource<'a> =
    dyn FnMut(&'static str) -> BoxFut<'a, Result<Box<dyn MerkleStore>, String>> + 'a;

/// the snapshot source a [`Boot::Reopen`] installs from: a module id → the
/// `(bytes, root)` to install, or `None` when there is nothing to install for
/// that id (its state lives in its store or on disk already).
pub type SnapshotSource<'a> =
    dyn FnMut(&'static str) -> BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>> + 'a;

/// the host-side disk substrates the odb-backed tenants open over.
pub struct Substrates {
    /// forge's git repo base dir.
    pub forge_repo: PathBuf,
    /// files' duckfs data dir (`<dir>/objects` + `<dir>/refs`).
    pub duckfs_dir: PathBuf,
    /// the node-local blob plane forge materializes pushed packs from.
    pub blobs: blobstore::BlobHandle,
}

/// the per-network genesis values the composer binds into module state.
pub struct Bindings<'a> {
    /// the invite namespace governance verifies tokens/join proofs against.
    pub invite: &'a [u8],
    /// the identity chain id identity/gateway scope their records to.
    pub chain_id: &'a str,
    /// the genesis validator set (32-byte ed25519 keys) valset seeds from.
    pub validators: &'a [Vec<u8>],
    /// module id → the sha256 of its genesis component: the code source is
    /// asked for these bytes, and lifecycle seeds its registry with them.
    pub code_hashes: &'a BTreeMap<String, [u8; 32]>,
}

/// how the composed modules come up.
pub enum Boot<'a> {
    /// fresh stores: the native registries seed from [`Bindings`], the
    /// store-backed network-bound tenants commit their `__config` record, and
    /// the Map tenants start empty.
    Genesis,
    /// reopened/synced stores: nothing re-seeds (a store already carries its
    /// genesis records), and `snapshots(id)` installs the Map (and, on the
    /// sync path, odb) tenants' state; `None` means "nothing to install".
    Reopen {
        /// the snapshot source consulted for every non-store-backed wasm tenant.
        snapshots: &'a mut SnapshotSource<'a>,
    },
}

/// compose `selection` in order: every id must be in the topology.
pub async fn compose(
    selection: &[&'static str],
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    mut boot: Boot<'_>,
) -> Result<Vec<Box<dyn Module>>, String> {
    let mut out = Vec::with_capacity(selection.len());
    for id in selection {
        let spec = TOPOLOGY
            .spec(id)
            .ok_or_else(|| format!("module {id} is not in the topology"))?;
        out.push(compose_module(spec, code, stores, substrates, bindings, &mut boot).await?);
    }
    Ok(out)
}

/// compose ONE module from its topology spec.
pub async fn compose_module(
    spec: &ModuleSpec,
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    match spec.code {
        Code::Native => native(spec, stores, bindings, boot).await,
        Code::Wasm => wasm(spec, code, stores, substrates, bindings, boot).await,
    }
}

/// the native system modules — all store-backed; the registries seed only at
/// genesis (their `finish_seed` is idempotent on a seeded store regardless).
async fn native(
    spec: &ModuleSpec,
    stores: &mut StoreSource<'_>,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    let is_genesis = matches!(boot, Boot::Genesis);
    let store = stores(spec.id).await?;
    match spec.id {
        "valset" => {
            let mut valset = valset::Valset::new(spec.id, store);
            if is_genesis {
                for key in bindings.validators {
                    valset
                        .seed(key.clone())
                        .await
                        .map_err(|e| format!("valset seed: {e}"))?;
                }
                valset
                    .finish_seed()
                    .await
                    .map_err(|e| format!("valset seed: {e}"))?;
            }
            Ok(Box::new(valset))
        }
        "lifecycle" => {
            let mut registry = lifecycle::Lifecycle::new(spec.id, store, "valset");
            if is_genesis {
                for (id, hash) in bindings.code_hashes {
                    registry
                        .seed(id.as_str(), hash.to_vec())
                        .await
                        .map_err(|e| format!("lifecycle seed {id}: {e}"))?;
                }
                registry
                    .finish_seed()
                    .await
                    .map_err(|e| format!("lifecycle seed: {e}"))?;
            }
            Ok(Box::new(registry))
        }
        "kv" => Ok(Box::new(kv::Kv::new(spec.id, store))),
        other => Err(format!(
            "native module {other} has no constructor in the composer"
        )),
    }
}

/// a wasm tenant: fetch its genesis component by hash, wrap it over the backing
/// the topology names, then install a reopen snapshot where one applies.
async fn wasm(
    spec: &ModuleSpec,
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    boot: &mut Boot<'_>,
) -> Result<Box<dyn Module>, String> {
    let hash = bindings
        .code_hashes
        .get(spec.id)
        .ok_or_else(|| format!("module {} has no genesis code hash", spec.id))?;
    let bytes = code.fetch(hash).await.ok_or_else(|| {
        format!(
            "code bytes absent for module {} (hash {}) — fail-closed",
            spec.id,
            crate::hex_bytes(hash)
        )
    })?;
    let loaded = match spec.backing {
        Backing::Map => WasmModule::from_bytes(spec.id, &bytes),
        Backing::Store => {
            let mut store = stores(spec.id).await?;
            let seeds_config = matches!(boot, Boot::Genesis);
            if seeds_config {
                seed_store_config(&mut *store, spec, bindings).await?;
            }
            WasmModule::with_store(spec.id, &bytes, store)
        }
        Backing::Odb => {
            let backing = open_odb(spec.id, substrates)?;
            WasmModule::with_odb(spec.id, &bytes, backing)
        }
    };
    let mut module = loaded.map_err(|e| format!("{} component loads: {e}", spec.id))?;
    if spec.committed_queries {
        module = module.with_committed_queries();
    }
    // a store-backed tenant's state IS its store: it never installs (and
    // `WasmModule::install` refuses), so the source is not even asked.
    let installs_snapshots = spec.backing != Backing::Store;
    if let Boot::Reopen { snapshots } = boot
        && installs_snapshots
        && let Some((snapshot, root)) = snapshots(spec.id).await?
    {
        module
            .install(&snapshot, root)
            .map_err(|e| format!("{} install: {e}", spec.id))?;
    }
    Ok(Box::new(module))
}

/// the odb-backed tenants' disk substrates, by id — `open` recovers each
/// substrate's committed position (files' refs envelope, forge's branches).
fn open_odb(
    id: &'static str,
    substrates: &Substrates,
) -> Result<Box<dyn wasm_host::OdbBacking>, String> {
    match id {
        "files" => {
            let backing = files::FilesOdbBacking::open(id, substrates.duckfs_dir.clone())
                .map_err(|e| format!("files open: {e}"))?;
            Ok(Box::new(backing))
        }
        "forge" => {
            let backing = forge::ForgeOdbBacking::open(
                id,
                substrates.forge_repo.clone(),
                substrates.blobs.clone(),
            )
            .map_err(|e| format!("forge open: {e}"))?;
            Ok(Box::new(backing))
        }
        other => Err(format!("odb module {other} has no backing in the composer")),
    }
}

/// commit a STORE-BACKED tenant's genesis `__config` record from the topology's
/// config keys; idempotent (a store already carrying one is left untouched).
async fn seed_store_config(
    store: &mut dyn MerkleStore,
    spec: &ModuleSpec,
    bindings: &Bindings<'_>,
) -> Result<(), String> {
    if spec.config.is_empty() {
        return Ok(());
    }
    let key = sdk::store_key(sdk::genesis_config::CONFIG_KEY);
    let already = store
        .get(&key)
        .await
        .map_err(|e| format!("{} genesis config read: {e}", spec.id))?;
    if already.is_some() {
        return Ok(());
    }
    let mut params: Vec<(&str, &[u8])> = Vec::with_capacity(spec.config.len());
    for config_key in spec.config {
        params.push((config_key, config_value(config_key, bindings)?));
    }
    let config = sdk::genesis_config::encode_config(&params);
    store
        .commit_batch(vec![(key, Some(config))])
        .await
        .map_err(|e| format!("{} genesis config seeds: {e}", spec.id))
}

/// the binding a topology config key resolves to; an unknown key is a
/// topology/composer drift, refused by name.
fn config_value<'a>(key: &str, bindings: &Bindings<'a>) -> Result<&'a [u8], String> {
    match key {
        topology::CONFIG_INVITE => Ok(bindings.invite),
        topology::CONFIG_CHAIN_ID => Ok(bindings.chain_id.as_bytes()),
        other => Err(format!("topology config key {other} has no binding")),
    }
}
