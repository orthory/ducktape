//! the ONE module composer: a topology selection + a code source + a store
//! source → a [`Host`]. genesis, restore, and statesync in `bin/node`, and the
//! noded/simnode daemons, all build their hosts here — and every wasm module a
//! host ever runs (a genesis tenant, a reopened or synced one, a live
//! admission) enters through [`wasm_module`]: the component's DECLARED shape
//! ([`WasmModule::declared_shape`] — backing, config keys, query mode) decides
//! the substrate it is wrapped over, never a table the host keeps.
//!
//! the module SET is the modules registry's: at block zero the founder's
//! bundle (which is exactly what the registry seeds from), at any later
//! boundary the registry's roster on the code it designates for that height
//! (`modules::code_at`). the natives (the compiled-in registries) are the
//! topology selection's, since they carry no registry entry.
//!
//! what varies per caller is injected, never branched on here: WHERE a store
//! comes from (`QmdbStore::init` on a fresh/reopened dir, `sync_from` at a
//! verified root) is the [`StoreSource`]; WHERE the component bytes come from
//! (a managed dir, the blob plane, the mesh) is the [`host::CodeSource`]; and
//! whether this is block zero over the founder's bundle or a boundary over
//! the registry's roster is the [`Boot`] mode.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use host::Host;
use sdk::{MerkleStore, Module, StateRoot};
use sha2::Digest as _;
use topology::{Code, TOPOLOGY};
use wasm_host::{Backing, CompiledModule, Shape, WasmModule};

/// a boxed, non-`Send` future (the host and every store are `!Send`).
pub type BoxFut<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + 'a>>;

/// the store source: a module id → its opened (or synced) authenticated store.
/// the caller decides the lifecycle; a refused open is an `Err` naming why.
pub type StoreSource<'a> =
    dyn FnMut(&str) -> BoxFut<'a, Result<Box<dyn MerkleStore>, String>> + 'a;

/// the snapshot source a [`Start::Resume`] installs from: a module id and its
/// declared backing → the `(bytes, root)` to install, or `None` when the
/// module's state at the boundary lives in its store or on its disk already.
/// never asked for a store-backed module (its state IS its store).
pub type SnapshotSource<'a> = dyn FnMut(&str, Backing) -> BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>>
    + 'a;

/// the host-side disk substrates the odb-backed tenants open over.
#[derive(Clone)]
pub struct Substrates {
    /// forge's git repo base dir.
    pub forge_repo: PathBuf,
    /// files' duckfs data dir (`<dir>/objects` + `<dir>/refs`).
    pub duckfs_dir: PathBuf,
    /// the node-local blob plane forge materializes pushed packs from.
    pub blobs: blobstore::BlobHandle,
}

/// the module ids this host provides an odb substrate for — the only ids a
/// component declaring [`Backing::Odb`] can run under.
const ODB_SUBSTRATES: &[&str] = &["files", "forge"];

/// the per-network values every composition binds into module state: the
/// invite namespace governance verifies tokens and join proofs against, and
/// the identity chain id identity/gateway/runs scope their records to. a
/// network-bound module's `__config` record is made of these. needed on
/// EVERY path, not just genesis: a module admitted after a checkpoint starts
/// fresh at restore and seeds its config then, exactly as it did live.
pub struct Bindings<'a> {
    pub invite: &'a [u8],
    pub chain_id: &'a str,
}

/// how the composed host comes up.
pub enum Boot<'a, 'b> {
    /// block zero. the native registries seed — valset from `validators`, the
    /// modules registry from `bundle` — and the wasm set IS `bundle` (module
    /// id → the sha256 of its genesis component: the code source is asked for
    /// these bytes, checked against the hash). every module starts
    /// [`Start::Fresh`]. PRECONDITION: `bundle`'s key set is EXACTLY the
    /// selection's wasm ids — every entry lands in the modules root, so a
    /// stray key would move the genesis root; [`compose`] refuses any drift
    /// by name.
    Genesis {
        validators: &'a [Vec<u8>],
        bundle: &'a BTreeMap<String, [u8; 32]>,
    },
    /// a checkpoint or manifest boundary at `height`. the natives reopen as
    /// committed, the wasm set is the modules registry's roster on the code it
    /// designates for `height`, and each module [`Start::Resume`]s from
    /// `snapshots` — except one whose first activation is past `height`,
    /// which the boundary predates: it starts fresh, as it did live.
    Reopen {
        height: u64,
        /// two lifetimes, like `StoreSource`'s `&mut StoreSource<'_>`: the
        /// borrow ends with the compose, the futures' lifetime is the
        /// closure's, so one source serves a compose AND a later
        /// [`wasm_module`].
        snapshots: &'a mut SnapshotSource<'b>,
    },
}

/// how ONE wasm module comes up.
pub enum Start<'a, 'b> {
    /// no state yet — a genesis tenant, a live admission, or a reopen before
    /// the module's first activation: its `__config` record seeds from the
    /// bindings (a store-backed module commits it into its merkle store, a
    /// map-backed one installs it as its initial map).
    Fresh,
    /// the module's state at a boundary: its store reopens or resyncs (the
    /// store source's business) and `snapshots(id, backing)` installs a
    /// map/odb image if the source has one.
    Resume {
        snapshots: &'a mut SnapshotSource<'b>,
    },
}

/// compose `selection` into a [`Host`]: every id must be in the topology; the
/// natives come from the selection, the wasm set from the boot mode (the
/// bundle at genesis, the registry at a reopen). the returned host carries no
/// module factory — the caller wires [`Admissions`] over its canonical
/// substrates.
pub async fn compose(
    selection: &[&'static str],
    code: &dyn host::CodeSource,
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    mut boot: Boot<'_, '_>,
) -> Result<Host, String> {
    let specs = selection
        .iter()
        .map(|id| {
            TOPOLOGY
                .spec(id)
                .ok_or_else(|| format!("module {id} is not in the topology"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut host = Host::new();
    for spec in &specs {
        match spec.code {
            Code::Native => register_new(&mut host, native(spec.id, stores, &boot).await?)?,
            Code::Wasm => {}
        }
    }
    let active = match &boot {
        Boot::Genesis { bundle, .. } => {
            check_bundle_keys(selection, bundle)?;
            bundle
                .iter()
                .map(|(id, hash)| ActiveCode {
                    id: id.clone(),
                    hash: *hash,
                    seat: Seat::Fresh,
                })
                .collect()
        }
        Boot::Reopen { height, .. } => registry_active_set(&host, *height).await?,
    };
    for entry in active {
        let bytes = fetch_code(code, &entry.id, &entry.hash).await?;
        let start = match &mut boot {
            Boot::Genesis { .. } => Start::Fresh,
            Boot::Reopen { snapshots, .. } => match entry.seat {
                Seat::Fresh => Start::Fresh,
                Seat::Resume => Start::Resume {
                    snapshots: &mut **snapshots,
                },
            },
        };
        let module = wasm_module(&entry.id, &bytes, stores, substrates, bindings, start).await?;
        register_new(&mut host, Box::new(module))?;
    }
    Ok(host)
}

/// register a module the host does not hold yet. dispatch addresses modules
/// by id, so a second module under one id — a registry roster entry colliding
/// with a native — is refused, never silently replaced.
fn register_new(host: &mut Host, module: Box<dyn Module>) -> Result<(), String> {
    let id = module.id();
    if host.module_root(&id).is_some() {
        return Err(format!("duplicate module id: {id}"));
    }
    host.register(module);
    Ok(())
}

/// one entry of the wasm set: the code `id` runs, and how it starts.
struct ActiveCode {
    id: String,
    hash: [u8; 32],
    seat: Seat,
}

/// how a module of the registry's roster starts at a boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Seat {
    /// the boundary predates the module's first activation: nothing to
    /// resume, it starts as it did live.
    Fresh,
    /// the module was live at the boundary, which holds its state.
    Resume,
}

/// the code `entry`'s module runs at the end of block `height` and how it
/// starts there: the registry's designated code (`modules::code_at` — a
/// pending swap armed at `height` wins, else the latest activation at or
/// before it), fresh when the module's first activation is past `height`,
/// resumed otherwise. `None` for a module registered but never activated:
/// nothing to run.
pub fn seat_at(entry: &modules::ModuleCode, height: u64) -> Option<([u8; 32], Seat)> {
    let hash: [u8; 32] = modules::code_at(entry, height)?.try_into().ok()?;
    let first_activation = entry.history.first().map(|a| a.height);
    let activated_by_height = first_activation.is_some_and(|activated| activated <= height);
    let seat = if activated_by_height {
        Seat::Resume
    } else {
        Seat::Fresh
    };
    Some((hash, seat))
}

/// the modules registry's roster at `height` ([`seat_at`] per entry). the
/// registry is a native of the selection, so a reopen over a selection
/// without it has no wasm set to read.
async fn registry_active_set(host: &Host, height: u64) -> Result<Vec<ActiveCode>, String> {
    let roster = host.module_status().await.ok_or_else(|| {
        "a reopen composes its wasm set from the modules registry, which this selection lacks"
            .to_string()
    })?;
    Ok(roster
        .into_iter()
        .filter_map(|entry| {
            let (hash, seat) = seat_at(&entry, height)?;
            Some(ActiveCode {
                id: entry.module_id,
                hash,
                seat,
            })
        })
        .collect())
}

/// the bundle's key set is EXACTLY the selection's wasm ids: every entry
/// seeds the modules registry (a stray key moves the genesis root) and every
/// wasm tenant needs one (a missing key cannot compose). named both ways.
fn check_bundle_keys(
    selection: &[&'static str],
    bundle: &BTreeMap<String, [u8; 32]>,
) -> Result<(), String> {
    let wanted: BTreeSet<&str> = TOPOLOGY.wasm_ids(selection).into_iter().collect();
    let given: BTreeSet<&str> = bundle.keys().map(String::as_str).collect();
    let missing: Vec<&str> = wanted.difference(&given).copied().collect();
    let extra: Vec<&str> = given.difference(&wanted).copied().collect();
    let is_exact = missing.is_empty() && extra.is_empty();
    if is_exact {
        return Ok(());
    }
    Err(format!(
        "the bundle must key exactly the selection's wasm modules: missing {missing:?}, extra {extra:?}"
    ))
}

/// the component bytes for `id` at `hash`, verified. a code source is a
/// lookup, not a guarantee: the bytes are re-hashed here exactly as the
/// host's swap path re-checks them, so a lying source (a dir keyed by
/// filename, a stale blob) can never seat code whose `code_hash()` disagrees
/// with the registry entry.
pub async fn fetch_code(
    code: &dyn host::CodeSource,
    id: &str,
    hash: &[u8; 32],
) -> Result<Vec<u8>, String> {
    let bytes = code.fetch(hash).await.ok_or_else(|| {
        format!(
            "code bytes absent for module {id} (hash {}) — fail-closed",
            crate::hex_bytes(hash)
        )
    })?;
    let matches_hash = sha2::Sha256::digest(&bytes)[..] == hash[..];
    if !matches_hash {
        return Err(format!(
            "module {id} code bytes do not match hash {} — fail-closed",
            crate::hex_bytes(hash)
        ));
    }
    Ok(bytes)
}

/// the native system modules — all store-backed; the registries seed only at
/// genesis (their `finish_seed` is idempotent on a seeded store regardless).
async fn native(
    id: &'static str,
    stores: &mut StoreSource<'_>,
    boot: &Boot<'_, '_>,
) -> Result<Box<dyn Module>, String> {
    let store = stores(id).await?;
    let founding = match boot {
        Boot::Genesis { validators, bundle } => Some((*validators, *bundle)),
        Boot::Reopen { .. } => None,
    };
    match id {
        "valset" => {
            let mut valset = valset::Valset::new(id, store);
            if let Some((validators, _)) = founding {
                for key in validators {
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
        "modules" => {
            let mut registry = modules::Modules::new(id, store, "valset");
            if let Some((_, bundle)) = founding {
                for (module_id, hash) in bundle {
                    registry
                        .seed(module_id.as_str(), hash.to_vec())
                        .await
                        .map_err(|e| format!("modules seed {module_id}: {e}"))?;
                }
                registry
                    .finish_seed()
                    .await
                    .map_err(|e| format!("modules seed: {e}"))?;
            }
            Ok(Box::new(registry))
        }
        "kv" => Ok(Box::new(kv::Kv::new(id, store))),
        other => Err(format!(
            "native module {other} has no constructor in the composer"
        )),
    }
}

/// the ONE wasm path: wrap `bytes` for `id` over the substrate its declared
/// shape names. a store-backed module opens its store through the source; an
/// odb-backed one opens the host substrate for its id; a map-backed one
/// starts from an empty map. then the start: fresh seeds the `__config`
/// record, a resume installs the boundary snapshot the source offers.
pub async fn wasm_module(
    id: &str,
    bytes: &[u8],
    stores: &mut StoreSource<'_>,
    substrates: &Substrates,
    bindings: &Bindings<'_>,
    start: Start<'_, '_>,
) -> Result<WasmModule, String> {
    let compiled =
        CompiledModule::compile(bytes).map_err(|e| format!("{id} component loads: {e}"))?;
    let shape = compiled.shape().clone();
    check_realizable(id, &shape)?;
    let is_fresh = matches!(start, Start::Fresh);
    let wrapped = match shape.backing {
        Backing::Map => compiled.over_map(id),
        Backing::Store => {
            let mut store = stores(id).await?;
            if is_fresh {
                seed_store_config(&mut *store, id, &shape, bindings).await?;
            }
            compiled.over_store(id, store)
        }
        Backing::Odb => compiled.over_odb(id, open_odb(id, substrates)?),
    };
    let mut module = wrapped.map_err(|e| format!("{id} component loads: {e}"))?;
    match start {
        // a MAP-backed network-bound module carries its `__config` in its
        // state map, so a fresh start INSTALLS the record the store-backed
        // twin committed above. it then rides snapshots and state-sync like
        // any other map entry (the guest's `save_state` never touches that
        // key), so only a fresh start seeds it — a resume's install below
        // replaces the whole map, config included.
        Start::Fresh => {
            let seeds_map_config = shape.backing == Backing::Map && !shape.config.is_empty();
            if seeds_map_config {
                let (bytes, root) = wasm_host::initial_state(&[(
                    sdk::genesis_config::CONFIG_KEY,
                    &encode_config(id, &shape, bindings)?,
                )]);
                module
                    .install(&bytes, root)
                    .map_err(|e| format!("{id} genesis config installs: {e}"))?;
            }
        }
        // a store-backed module's state IS its store: it never installs (and
        // `WasmModule::install` refuses), so the source is not even asked.
        Start::Resume { snapshots } => {
            let installs_snapshots = shape.backing != Backing::Store;
            if installs_snapshots && let Some((snapshot, root)) = snapshots(id, shape.backing).await? {
                module
                    .install(&snapshot, root)
                    .map_err(|e| format!("{id} install: {e}"))?;
            }
        }
    }
    Ok(module)
}

/// can THIS host run a component of `shape` under `id`? an odb declaration
/// needs a host substrate for the id, and every config key must be one the
/// network binds. the same check a validator applies before it signals a
/// swap ready, so an admission the boundary could not realize is refused
/// before it is ever scheduled, never at the boundary of every validator.
pub fn check_realizable(id: &str, shape: &Shape) -> Result<(), String> {
    let declares_odb = shape.backing == Backing::Odb;
    let has_odb_substrate = ODB_SUBSTRATES.contains(&id);
    if declares_odb && !has_odb_substrate {
        return Err(no_odb_substrate(id));
    }
    for key in &shape.config {
        require_config_key(id, key)?;
    }
    Ok(())
}

fn no_odb_substrate(id: &str) -> String {
    format!(
        "module {id} declares an odb backing, but this host provides an odb substrate only for {ODB_SUBSTRATES:?}"
    )
}

/// the odb-backed tenants' disk substrates, by id — `open` recovers each
/// substrate's committed position (files' refs envelope, forge's branches).
fn open_odb(
    id: &str,
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
        other => Err(no_odb_substrate(other)),
    }
}

/// commit a STORE-BACKED module's `__config` record from its declared config
/// keys; idempotent (a store already carrying one is left untouched).
async fn seed_store_config(
    store: &mut dyn MerkleStore,
    id: &str,
    shape: &Shape,
    bindings: &Bindings<'_>,
) -> Result<(), String> {
    if shape.config.is_empty() {
        return Ok(());
    }
    let key = sdk::store_key(sdk::genesis_config::CONFIG_KEY);
    let already = store
        .get(&key)
        .await
        .map_err(|e| format!("{id} genesis config read: {e}"))?;
    if already.is_some() {
        return Ok(());
    }
    let config = encode_config(id, shape, bindings)?;
    store
        .commit_batch(vec![(key, Some(config))])
        .await
        .map_err(|e| format!("{id} genesis config seeds: {e}"))
}

/// this module's `__config` bytes: every declared config key resolved against
/// the network bindings. the codec wants strictly increasing keys, so the
/// declaration's order does not matter and a duplicate collapses.
fn encode_config(id: &str, shape: &Shape, bindings: &Bindings<'_>) -> Result<Vec<u8>, String> {
    let keys: BTreeSet<&str> = shape.config.iter().map(String::as_str).collect();
    let mut params: Vec<(&str, &[u8])> = Vec::with_capacity(keys.len());
    for key in keys {
        params.push((key, config_value(id, key, bindings)?));
    }
    Ok(sdk::genesis_config::encode_config(&params))
}

/// the binding a declared config key resolves to; an unknown key is refused
/// by name (a component asking for a parameter no network binds).
fn config_value<'a>(id: &str, key: &str, bindings: &Bindings<'a>) -> Result<&'a [u8], String> {
    match key {
        sdk::genesis_config::INVITE => Ok(bindings.invite),
        sdk::genesis_config::CHAIN_ID => Ok(bindings.chain_id.as_bytes()),
        other => Err(unbound_config_key(id, other)),
    }
}

/// the keys [`config_value`] resolves, checked without a binding in hand.
fn require_config_key(id: &str, key: &str) -> Result<(), String> {
    let known = key == sdk::genesis_config::INVITE || key == sdk::genesis_config::CHAIN_ID;
    if known {
        return Ok(());
    }
    Err(unbound_config_key(id, key))
}

fn unbound_config_key(id: &str, key: &str) -> String {
    format!(
        "module {id} declares config key {key:?}, which no network binds (known: {:?}, {:?})",
        sdk::genesis_config::CHAIN_ID,
        sdk::genesis_config::INVITE
    )
}

/// the [`host::ModuleFactory`] a composed host carries: a post-genesis
/// admission (governance `RegisterModule` → modules `ScheduleRegister`)
/// builds its module through [`wasm_module`] at the activation boundary —
/// starting fresh over a store the canonical source opens under its id, the
/// node's substrates, and the network bindings. the constructor twin of the
/// code source, and the same path a genesis tenant took at block zero.
pub struct Admissions {
    context: commonware_runtime::tokio::Context,
    substrates: Substrates,
    invite: Vec<u8>,
    chain_id: String,
}

impl Admissions {
    /// over the node's CANONICAL substrates and store root (a sync attempt's
    /// scratch dirs are never a home for a module admitted later). the
    /// runtime hands out owned contexts only as labeled children; a store's
    /// partitions are named by module id alone, so a child opens the same
    /// store the boot context would.
    pub fn new(
        context: &commonware_runtime::tokio::Context,
        substrates: &Substrates,
        bindings: &Bindings<'_>,
    ) -> Self {
        use commonware_runtime::Supervisor as _;
        Self {
            context: context.child("admissions"),
            substrates: substrates.clone(),
            invite: bindings.invite.to_vec(),
            chain_id: bindings.chain_id.to_string(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl host::ModuleFactory for Admissions {
    async fn instantiate(&self, id: &str, bytes: &[u8]) -> Result<host::Admitted, sdk::Error> {
        let bindings = Bindings {
            invite: &self.invite,
            chain_id: &self.chain_id,
        };
        let mut stores = crate::bundle::qmdb_stores(&self.context);
        let seated =
            wasm_module(id, bytes, &mut stores, &self.substrates, &bindings, Start::Fresh).await;
        let refusal = match seated {
            Ok(module) => return Ok(host::Admitted::Module(Box::new(module))),
            Err(refusal) => refusal,
        };
        // ONLY now: do these bytes even speak the module ABI? A `ducktape:
        // module` this build refused stays fail-closed (an older binary must
        // never silently seat a different registry set than its peers); bytes
        // that are no module at all are another plane's record, and this
        // boundary is not the plane that realizes them. The extra compile is
        // paid on the refusal path alone, and the host latches the answer.
        let is_a_module = wasm_host::speaks_module_abi(bytes);
        match is_a_module {
            true => Err(sdk::Error::Module(refusal)),
            false => Ok(host::Admitted::ForeignAbi),
        }
    }
}
