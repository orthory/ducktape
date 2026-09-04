//! Application-host construction, restoration, and state synchronization.
//!
//! This module owns the three host lifecycles — genesis, checkpoint restore,
//! and state sync — over the ONE composer ([`noded::compose`]). Each lifecycle
//! injects only what differs: WHERE its stores come from (a fresh/reopened
//! `QmdbStore::init`, or a `sync_from` at a verified root) and WHERE its
//! snapshots come from (the checkpoint, or the peer's snapshot lane). The
//! module SET and every module's shape are the topology's, and the genesis
//! wasm is the workspace genesis file's — every component verified against
//! the descriptor's hashes and seeded into the node's blob plane, every index
//! guest converged into the index, never embedded in the binary. The live
//! node loop only consumes the three lifecycle operations and the output
//! adapter exported below.

use commonware_cryptography::ed25519;
use commonware_runtime::Supervisor as _;
use duckfs_disk::SyncScratch;
use files::Files;
use host::Host;
use noded::bundle::{host_from, qmdb_stores};
use noded::compose::{Bindings, Boot, BoxFut, Substrates, compose, compose_module};
use recovery::Manifest;
use sdk::StateRoot;
use sha2::Digest as _;
use statesync::{
    fetch_snapshot,
    qmdb::{QmdbStore, RemoteQmdbResolver},
};
use topology::{Backing, PRODUCTION, TOPOLOGY};
use wasm_host::WasmModule;

use noded::{IndexGuests, converge_index_guests};

use crate::config::{
    Genesis, GenesisModules, GenesisSource, component_path, hex_bytes, install_genesis,
    modules_path, verify_genesis,
};
use crate::constants::MODULE_IDS;
use crate::util::hex;

/// what a reopen installs into a Map tenant: `(snapshot bytes, root)`, or
/// `None` when the tenant's state is already on its own disk substrate.
type Snapshot = Option<(Vec<u8>, StateRoot)>;

/// a snapshot source keyed by a modules-registry id (a `String`, not a
/// topology `&'static str`) — what [`adopt_admitted_modules`] installs from.
type AdmittedSnapshotSource<'a> =
    dyn for<'id> FnMut(&'id str) -> BoxFut<'id, Result<Snapshot, String>> + 'a;

/// Consensus-visible network names shared by genesis, restore, and state sync.
#[derive(Clone, Copy)]
pub(super) struct NetworkBindings<'a> {
    pub(super) invite: &'a [u8],
    pub(super) identity_chain_id: &'a str,
}

/// the node-local substrates every host construction composes over — genesis,
/// restore, and state sync alike: forge's git repo, files' duckfs dir, the
/// blob store the genesis components are hydrated into, and the derived index
/// the genesis's guests converge into.
pub(super) struct NodeSubstrates<'a> {
    pub(super) forge_repo: &'a std::path::Path,
    pub(super) duckfs_dir: &'a std::path::Path,
    pub(super) blobs: blobstore::BlobHandle,
    pub(super) index: &'a indexer::IndexStore,
}

/// the blobstore-backed [`host::CodeSource`]: component bytes — the genesis
/// file's, seeded by [`hydrate_genesis`], and a code swap's, staged there before
/// the governance schedule exactly like a forge Push packfile — are
/// content-addressed chunks on the node's blob plane. a hash the store lacks
/// is a `None` — the boundary fails closed rather than forking.
pub(super) struct BlobCodeSource(pub(super) std::sync::Arc<dyn blobstore::Blobs>);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for BlobCodeSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        self.0.get_chunk(&digest)
    }

    fn origin(&self) -> &'static str {
        "blob_local"
    }
}

/// what [`hydrate_from_disk`] found on disk.
enum Hydrated {
    /// every component is in the blob store and every index guest converged.
    Installed,
    /// the workspace genesis file is absent — a joiner before its first
    /// fetch — and nothing was installed. `hash` is the descriptor's pin the
    /// fetch asks the mesh for.
    GenesisAbsent { file: std::path::PathBuf, hash: [u8; 32] },
}

/// install the genesis this node holds on disk: every component into the
/// (persistent) blob store, verified by name against the descriptor's hash;
/// the workspace genesis file itself as a chunk keyed by its own hash, so this
/// node serves it to the next joiner; and every index guest into its module's
/// index database. Idempotent: a chunk the store already holds is not re-put,
/// and an index database already holding its guest converges for free.
fn hydrate_from_disk(
    blobs: &blobstore::BlobHandle,
    index: &indexer::IndexStore,
    genesis: &GenesisModules,
) -> Result<Hydrated, String> {
    let guests = match &genesis.source {
        GenesisSource::FoundingSet(dir) => {
            seed_founding_set(blobs, dir, &genesis.hashes)?;
            IndexGuests::from_dir(dir, MODULE_IDS)?
        }
        GenesisSource::Workspace { file, hash } => {
            let Some(loaded) = seed_workspace_genesis(blobs, file, hash, &genesis.hashes)? else {
                return Ok(Hydrated::GenesisAbsent {
                    file: file.clone(),
                    hash: *hash,
                });
            };
            // the verified genesis, unpacked beside network.toml as bare files
            let workspace = file
                .parent()
                .ok_or_else(|| format!("{} has no workspace directory", file.display()))?;
            loaded.materialize(&modules_path(workspace))?;
            IndexGuests::from_genesis(&loaded, MODULE_IDS)?
        }
    };
    converge_index_guests(index, &guests)?;
    Ok(Hydrated::Installed)
}

/// [`hydrate_from_disk`] for a node with no mesh to fetch from — a founder, a
/// member, or a workspace reopening its checkpoint: an absent genesis is a
/// refusal naming the file and how it gets there.
pub(super) fn hydrate_genesis(
    blobs: &blobstore::BlobHandle,
    index: &indexer::IndexStore,
    genesis: &GenesisModules,
) -> Result<(), String> {
    match hydrate_from_disk(blobs, index, genesis)? {
        Hydrated::Installed => Ok(()),
        Hydrated::GenesisAbsent { file, .. } => Err(format!(
            "{} is missing — fail-closed. a genesis member boots from its own genesis: \
             re-run `ducktape node join <invite> --genesis <file>` with the founder's \
             genesis file, or re-found with `ducktape node init`",
            file.display()
        )),
    }
}

/// the joiner's twin of [`hydrate_genesis`]: a workspace that lacks its
/// genesis fetches it off the mesh by the descriptor's pin (the ranged blob
/// lane — every node seeded its own genesis file into its blob store as a
/// chunk keyed by that hash), installs it beside `network.toml`, then
/// hydrates exactly as a member does. Runs inside the replica's forever-retry
/// loop, so the fetch is announced on attempt 1, never per try.
pub(super) async fn fetch_and_hydrate_genesis<
    C: statesync::SyncClient + crate::blob_fetch::SourceRotate,
>(
    client: &C,
    blobs: &blobstore::BlobHandle,
    index: &indexer::IndexStore,
    genesis: &GenesisModules,
    attempt: usize,
) -> Result<(), String> {
    let Hydrated::GenesisAbsent { hash, .. } = hydrate_from_disk(blobs, index, genesis)? else {
        return Ok(());
    };
    let first_attempt = attempt <= 1;
    if first_attempt {
        tracing::info!(
            target: "ducktape::boot",
            genesis = %hex_bytes(&hash),
            "genesis absent from the workspace; fetching it from the mesh"
        );
    }
    crate::blob_fetch::fetch_blob(
        client,
        blobs,
        &hash,
        crate::constants::MAX_MODULE_CODE_BYTES,
        crate::constants::BLOB_FETCH_ATTEMPTS,
    )
    .await
    .map_err(|e| format!("fetch genesis {}: {e}", hex_bytes(&hash)))?;
    // the fetch landed the genesis in the blob store under its pin, which is
    // exactly where a missing workspace file is rewritten from.
    match hydrate_from_disk(blobs, index, genesis)? {
        Hydrated::Installed => Ok(()),
        Hydrated::GenesisAbsent { hash, .. } => Err(format!(
            "fetched genesis {} is not in the blob store",
            hex_bytes(&hash)
        )),
    }
}

/// seed the blob store from a workspace genesis: `None` when this node holds
/// no genesis at all; otherwise the decoded genesis, every component the
/// store lacked put (each verified against `want` first — a tampered file
/// refuses by module id, and the whole file by its hash) and the file itself
/// put as a chunk keyed by `hash`. The blob store holds every genesis this
/// node ever installed or fetched under its pin, so the file is that chunk's
/// readable copy: a missing file with the chunk present is rewritten from it.
fn seed_workspace_genesis(
    blobs: &blobstore::BlobHandle,
    file: &std::path::Path,
    hash: &[u8; 32],
    want: &std::collections::BTreeMap<String, [u8; 32]>,
) -> Result<Option<Genesis>, String> {
    let bytes = match read_optional(file)? {
        Some(bytes) => bytes,
        None => {
            let Some(bytes) = blobs.get_chunk(hash) else {
                return Ok(None);
            };
            let workspace = file
                .parent()
                .ok_or_else(|| format!("{} has no workspace directory", file.display()))?;
            install_genesis(workspace, hash, want, &bytes)?;
            tracing::info!(
                target: "ducktape::boot",
                genesis = %hex_bytes(hash),
                file = %file.display(),
                "genesis file rewritten from the blob store"
            );
            bytes
        }
    };
    let genesis = verify_genesis(&bytes, hash, want).map_err(|e| format!("{}: {e}", file.display()))?;
    let mut seeded = 0usize;
    for (id, digest) in want {
        if blobs.has_chunk(digest) {
            continue;
        }
        // verified above: every id in `want` is a component hashing to `digest`.
        let component = genesis.component(id).expect("verified genesis carries every module");
        blobs.put_chunk(component.to_vec());
        seeded += 1;
    }
    if !blobs.has_chunk(hash) {
        blobs.put_chunk(bytes);
    }
    // a lifecycle fact: bytes entered the store (a restart over a seeded store
    // seeds nothing and says nothing).
    let seeded_any = seeded > 0;
    if seeded_any {
        tracing::info!(
            target: "ducktape::boot",
            modules = want.len(),
            seeded,
            genesis = %file.display(),
            "genesis seeded into the blob store"
        );
    }
    Ok(Some(genesis))
}

/// seed the blob store from a founding set — the dev shape, whose files ARE
/// its genesis code set: every component the store lacks is read from
/// `<dir>/<id>.component.wasm`, verified against `want`, and put. A missing
/// file refuses by module id and path (the set was hashed at resolve, so an
/// absence now is a set that changed under the node).
fn seed_founding_set(
    blobs: &blobstore::BlobHandle,
    dir: &std::path::Path,
    want: &std::collections::BTreeMap<String, [u8; 32]>,
) -> Result<(), String> {
    let mut seeded = 0usize;
    for (id, digest) in want {
        if blobs.has_chunk(digest) {
            continue;
        }
        let path = component_path(dir, id);
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("module {id}: read {}: {e} — fail-closed", path.display()))?;
        let got: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        let matches_descriptor = got == *digest;
        if !matches_descriptor {
            return Err(format!(
                "module {id}: {} hashes to {} but the descriptor says {} — fail-closed",
                path.display(),
                hex_bytes(&got),
                hex_bytes(digest)
            ));
        }
        blobs.put_chunk(bytes);
        seeded += 1;
    }
    let seeded_any = seeded > 0;
    if seeded_any {
        tracing::info!(
            target: "ducktape::boot",
            modules = want.len(),
            seeded,
            founding_set = %dir.display(),
            "founding set seeded into the blob store"
        );
    }
    Ok(())
}

/// a file's bytes; `None` when there is no file at `path`. any other failure
/// names the path (the operator's next move is to look at that directory).
fn read_optional(path: &std::path::Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => Ok(None),
            other => Err(format!("read {} ({other:?}): {e}", path.display())),
        },
    }
}

/// the host-side disk substrates the odb-backed tenants open over.
fn disk_substrates(
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    blobs: blobstore::BlobHandle,
) -> Substrates {
    Substrates {
        forge_repo: forge_repo.to_path_buf(),
        duckfs_dir: duckfs_dir.to_path_buf(),
        blobs,
    }
}

/// the per-network genesis values: the descriptor's code hashes (EXACTLY the
/// production wasm set — the composer refuses any drift by name) plus the
/// network bindings the network-bound tenants seed as their `__config` record.
fn bindings<'a>(
    net: &NetworkBindings<'a>,
    validators: &'a [Vec<u8>],
    genesis: &'a GenesisModules,
) -> Bindings<'a> {
    Bindings {
        invite: net.invite,
        chain_id: net.identity_chain_id,
        validators,
        code_hashes: &genesis.hashes,
    }
}

/// the PRODUCTION module set at block zero — genesis state, identical on every
/// node (a different set, or different component bytes, composes a different
/// root-hash and the network forks at genesis): the topology's production
/// selection over the bundle's components, valset seeded with the genesis
/// validators and the modules registry with the descriptor's code hashes.
pub(super) async fn genesis_host(
    context: &commonware_runtime::tokio::Context,
    genesis_validators: &[ed25519::PublicKey],
    net: NetworkBindings<'_>,
    substrates: NodeSubstrates<'_>,
    genesis: &GenesisModules,
) -> Result<Host, String> {
    let NodeSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
        index,
    } = substrates;
    hydrate_genesis(&blobs, index, genesis)?;
    let validators: Vec<Vec<u8>> = genesis_validators
        .iter()
        .map(|k| k.as_ref().to_vec())
        .collect();
    let code = BlobCodeSource(std::sync::Arc::new(blobs.clone()));
    let mut stores = qmdb_stores(context);
    let modules = compose(
        PRODUCTION,
        &code,
        &mut stores,
        &disk_substrates(forge_repo, duckfs_dir, blobs),
        &bindings(&net, &validators, genesis),
        Boot::Genesis,
    )
    .await
    .map_err(|e| format!("genesis compose: {e}"))?;
    host_from(modules).map_err(|e| format!("genesis host: {e}"))
}

/// the RESTORE twin of [`genesis_host`]: the disk substrates (qmdb stores,
/// forge's git repo, files' duckfs dir) reopen themselves at their own
/// committed positions; the Map cohort installs its checkpoint snapshots,
/// root-checked. every wasm tenant is rebuilt on its GENESIS component here —
/// recovery's boot-time code reconciliation (`Host::realize_module_swaps`)
/// swaps it to the committed active code when the registry has moved past
/// genesis. the network bindings are already committed store records, so
/// none are re-seeded. the recovery replay then rolls everything forward to
/// the journal tip.
pub(super) async fn restore_host(
    context: &commonware_runtime::tokio::Context,
    manifest: &Manifest,
    substrates: NodeSubstrates<'_>,
    genesis: &GenesisModules,
) -> Result<Host, String> {
    let NodeSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
        index,
    } = substrates;
    hydrate_genesis(&blobs, index, genesis)?;
    let code = BlobCodeSource(std::sync::Arc::new(blobs.clone()));
    let mut stores = qmdb_stores(context);
    let mut snapshots = |id: &'static str| -> BoxFut<'_, Result<Snapshot, String>> {
        let got = restore_snapshot(manifest, id);
        Box::pin(async move { got })
    };
    let net = NetworkBindings {
        invite: &[],
        identity_chain_id: "",
    };
    let modules = compose(
        PRODUCTION,
        &code,
        &mut stores,
        &disk_substrates(forge_repo, duckfs_dir, blobs),
        &bindings(&net, &[], genesis),
        Boot::Reopen {
            snapshots: &mut snapshots,
        },
    )
    .await?;
    let mut host = host_from(modules).map_err(|e| format!("restore host: {e}"))?;
    // a genesis checkpoint has applied nothing: an admitted module seats its
    // first code and the replay moves it forward from there.
    let checkpoint_height = manifest.height.unwrap_or(0);
    adopt_admitted_modules(&mut host, &code, checkpoint_height, &mut |id| {
        let got = admitted_restore_snapshot(manifest, id);
        Box::pin(async move { got })
    })
    .await?;
    Ok(host)
}

/// an admitted module's checkpoint state. checkpoints are periodic while the
/// the modules registry store is per-block durable and reopens AHEAD of the checkpoint,
/// so a registry id the checkpoint never captured is an admission that
/// activated after it: register the module EMPTY (its whole history is
/// post-checkpoint and replay rebuilds it — exactly what
/// `realize_module_swaps`' factory arm did). an entry the checkpoint HAS but
/// cannot complete stays an error.
fn admitted_restore_snapshot(manifest: &Manifest, id: &str) -> Result<Snapshot, String> {
    let admitted_after_checkpoint = manifest.snapshot(id).is_none();
    if admitted_after_checkpoint {
        return Ok(None);
    }
    manifest_snapshot(manifest, id).map(Some)
}

/// what a checkpoint restore installs for a tenant: a Map tenant's snapshot
/// (the checkpoint captures it); an odb tenant (files, forge) reopens its own
/// disk substrate at its committed position and installs nothing.
fn restore_snapshot(manifest: &Manifest, id: &str) -> Result<Snapshot, String> {
    let spec = TOPOLOGY
        .spec(id)
        .ok_or_else(|| format!("module {id} is not in the topology"))?;
    match spec.backing {
        Backing::Map => manifest_snapshot(manifest, id).map(Some),
        Backing::Store | Backing::Odb => Ok(None),
    }
}

/// the checkpoint's `(snapshot, root)` for `id`; both must be present.
fn manifest_snapshot(manifest: &Manifest, id: &str) -> Result<(Vec<u8>, StateRoot), String> {
    let bytes = manifest
        .snapshot(id)
        .ok_or_else(|| format!("checkpoint has no snapshot for module {id}"))?;
    let root = manifest
        .root(id)
        .ok_or_else(|| format!("checkpoint has no root for module {id}"))?;
    Ok((bytes.to_vec(), root))
}

/// register every module the modules registry admitted post-genesis — an id
/// the topology selection did not compose — on the code the registry
/// designates for `checkpoint_height` (`modules::code_at`). the registry is
/// per-block durable and reopens AHEAD of the checkpoint, so its ACTIVE hash
/// may be a swap the replay has yet to reach; seating the checkpoint's code
/// lets replay's `realize_module_swaps` move the module forward through the
/// same swap points the live node took. Map-backed by construction (admission
/// instantiates `from_bytes`), so its state is `snapshot(id)` — the
/// checkpoint's or the peer's.
async fn adopt_admitted_modules(
    host: &mut Host,
    code: &dyn host::CodeSource,
    checkpoint_height: u64,
    snapshot: &mut AdmittedSnapshotSource<'_>,
) -> Result<(), String> {
    let Some(registry) = host.module_status().await else {
        return Ok(());
    };
    for m in registry {
        let already_composed = host.module_root(&m.module_id).is_some();
        if already_composed {
            continue;
        }
        // an admission that has not reached its boundary has no code yet.
        let Some(code_hash) = modules::code_at(&m, checkpoint_height) else {
            continue;
        };
        let bytes = code.fetch(code_hash).await.ok_or_else(|| {
            format!(
                "code bytes absent for admitted module {} (hash {}) — fail-closed",
                m.module_id,
                hex_bytes(code_hash)
            )
        })?;
        let mut module = WasmModule::from_bytes(m.module_id.as_str(), &bytes)
            .map_err(|e| format!("admitted module {} loads: {e}", m.module_id))?;
        if let Some((snap, root)) = snapshot(&m.module_id).await? {
            module
                .install(&snap, root)
                .map_err(|e| format!("admitted module {} install: {e}", m.module_id))?;
        }
        host.register(Box::new(module));
    }
    Ok(())
}

/// the object-store ([`statesync::ObjectFetch`]) adapter over the live `files`
/// module: the statesync possession driver owns the loop + the full-possession
/// gate, this owns the duckfs `serve_sync` wire (refs image + `GetObjects`).
///
/// SCRATCH NAMESPACE (#219): like the qmdb modules — whose `sync_from` lands
/// under an ATTEMPT-scoped runtime child (`{name}_scratch_a{n}`) — the module
/// this adapter wraps is opened over `duckfs_disk::SyncScratch`'s attempt-scoped
/// scratch dir, NEVER the canonical `duckfs_dir`. the canonical dir is written
/// only by the verified promotion after `sync_all_modules`' composite root-hash
/// gate, so a failed join leaves it byte-untouched.
struct FilesOdb<'a>(&'a mut Files);

impl statesync::ObjectFetch for FilesOdb<'_> {
    fn refs_request(&self) -> Vec<u8> {
        duckfs_core::encode_get_refs()
    }

    fn install_refs(&mut self, reply: &[u8], root: StateRoot, height: u64) -> Result<(), String> {
        let bytes = duckfs_core::decode_refs_reply(reply)?;
        // persist the refs envelope at the SYNCED boundary height so a restart
        // right after the join resumes replay from the boundary, not genesis.
        self.0
            .install(&bytes, root, height)
            .map_err(|e| e.to_string())
    }

    fn missing_request(&self, limit: usize) -> Result<Option<Vec<u8>>, String> {
        let ids = self.0.missing_objects(limit).map_err(|e| e.to_string())?;
        if ids.is_empty() {
            return Ok(None);
        }
        Ok(Some(duckfs_core::encode_get_objects(&ids)))
    }

    fn ingest(&mut self, reply: &[u8]) -> Result<usize, String> {
        let batch = duckfs_core::decode_objects_reply(reply)?;
        let landed = batch.len();
        self.0.ingest_objects(&batch).map_err(|e| e.to_string())?;
        Ok(landed)
    }

    fn possession_complete(&self) -> Result<bool, String> {
        self.0.possession_complete().map_err(|e| e.to_string())
    }
}

/// the sync SNAPSHOT lane: every Map tenant plus forge (its refs image rides
/// the snapshot lane over the host-side git substrate). files is
/// possession-synced outside the composer, and a store-backed tenant's state
/// arrives through its store.
fn rides_the_sync_snapshot_lane(id: &str) -> bool {
    let is_forge = id == "forge";
    let is_map = TOPOLOGY
        .spec(id)
        .is_some_and(|spec| spec.backing == Backing::Map);
    is_forge || is_map
}

/// rebuild EVERY production module from a peer's statesync service at
/// `manifest`'s boundary and compose them into a [`Host`], verified against
/// the manifest's root-hash. the disk substrates land under their canonical
/// ids in this process's storage root — this IS the node's state afterwards,
/// not a scratch copy. `attempt` disambiguates runtime child labels across
/// retries (a busy source moves its qmdb targets past the captured boundary;
/// the caller refetches the manifest and tries again, and metrics labels
/// must not collide). the wasm tenants join on their GENESIS components — a
/// post-swap network's committed active hash differs, and the joiner's first
/// code reconciliation (before it applies any block) swaps them to the
/// committed components, fetched off the blob plane.
pub(super) async fn sync_all_modules<C: statesync::SyncClient + crate::blob_fetch::SourceRotate>(
    context: &commonware_runtime::tokio::Context,
    client: &C,
    manifest: &statesync::Manifest,
    substrates: NodeSubstrates<'_>,
    attempt: usize,
    genesis: &GenesisModules,
) -> Result<Host, String> {
    let NodeSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
        index,
    } = substrates;
    // a joiner's workspace holds no genesis until this fetches it (`node
    // join` writes none for a non-member): the whole file comes off the mesh
    // by the descriptor's pin and installs beside network.toml, so after this
    // every genesis component is in the blob store and every index guest is
    // in the index. `FetchingCodeSource` below still covers a component the
    // code registry swapped in after genesis; the composer sha256-checks
    // every fetched byte against the committed hash either way.
    fetch_and_hydrate_genesis(client, &blobs, index, genesis, attempt).await?;
    let entry_root = |module: &str| -> Result<StateRoot, String> {
        Ok(manifest
            .entry(module)
            .ok_or_else(|| format!("module {module} missing from the manifest"))?
            .root)
    };
    let scratch_context = context.child(Box::leak(
        format!("sync_scratch_a{attempt}").into_boxed_str(),
    ));
    let child_label = |name: &str| -> &'static str {
        Box::leak(format!("{name}_scratch_a{attempt}").into_boxed_str())
    };
    let pinned_target = |module: &'static str| -> Result<statesync::qmdb::SyncTarget, String> {
        let entry = manifest
            .entry(module)
            .ok_or_else(|| format!("module {module} missing from the manifest"))?;
        let pinned = entry
            .resolver_target
            .as_ref()
            .ok_or_else(|| format!("module {module} missing pinned resolver target"))?;
        pinned.to_sync_target().map_err(|e| format!("{module} {e}"))
    };

    // resolver lane: adopt the manifest's pinned target, then fetch only
    // boundary-scoped op batches through the remote resolver.
    let fetch_target = |module: &'static str| {
        let resolver = RemoteQmdbResolver::new(client.clone(), manifest.boundary_id(), module);
        async move {
            let target = pinned_target(module)?;
            let root = entry_root(module)?;
            if StateRoot(target.root.0) != root {
                return Err(format!(
                    "{module} pinned target root does not match the manifest root"
                ));
            }
            Ok::<_, String>((target, resolver))
        }
    };
    // snapshot lane: chunked bytes from the captured boundary, install gated
    // on the manifest root (verify-then-adopt inside each module). by value:
    // an admitted module's id comes off the modules registry, not the
    // topology.
    let snapshot_of = |module: &str| {
        let client = client.clone();
        let boundary = manifest.boundary_id();
        let root = entry_root(module);
        let module = module.to_string();
        async move {
            let root = root?;
            let bytes = fetch_snapshot(&client, boundary, &module)
                .await
                .map_err(|e| format!("{module} snapshot: {e}"))?;
            Ok::<_, String>((bytes, root))
        }
    };

    // files is a duckfs-odb resolver module: its refs image AND its
    // content-addressed objects both ride the Module/`serve_sync` lane. a fresh
    // joiner's odb is EMPTY, so install the boundary refs (root-verified) at the
    // sync-target height and then loop GetObjects to full object possession —
    // the snapshot lane would leave this node refs-only (every file listed, not
    // one byte readable). the sync lands in an ATTEMPT-scoped scratch dir
    // (`duckfs_scratch_a{attempt}`, mirroring the qmdb scratch namespaces);
    // the canonical `duckfs_dir` is written only by the verified promotion
    // after the composite root-hash gate below (#219).
    let files_scratch =
        SyncScratch::prepare(duckfs_dir, attempt).map_err(|e| format!("duckfs scratch: {e}"))?;
    // possession is node-local verification machinery, OFF consensus: drive it
    // with the NATIVE `Files` exactly as before (the joiner methods — install /
    // missing_objects / ingest / possession_complete — live on `Files`, not the
    // odb backing). it installs the boundary refs (at the sync-target height) and
    // ingests objects, all fsynced durably to the scratch dir.
    let mut files_possession = Files::open("files", files_scratch.dir().to_path_buf())
        .map_err(|e| format!("duckfs open: {e}"))?;
    let files_root = entry_root("files")?;
    let files_lane = statesync::ClientModuleLane::new(client.clone(), manifest.boundary_id());
    statesync::sync_object_possession(
        &files_lane,
        "files",
        files_root,
        manifest.height,
        &mut FilesOdb(&mut files_possession),
        duckfs_core::MAX_SYNC_IDS,
    )
    .await
    .map_err(|e| format!("files sync: {e}"))?;
    // possession wrote the synced refs envelope + every object durably to the
    // scratch dir, so drop that native handle and compose the ROOT-CONTINUOUS
    // wasm files tenant over the SAME scratch dir for the composite root-hash gate
    // below — `FilesOdbBacking::open` recovers exactly the possession-synced refs
    // (and its `durable_commit_height` from the envelope), so `root() =
    // sha256(refs_bytes)` certifies against the manifest's files root.
    drop(files_possession);

    // the composer's three sources on this path: code from the blob plane (the
    // mesh behind it — a joiner whose bundle trails a committed component
    // fetches it), every store rebuilt at the manifest's pinned target
    // (merkle-verified against the committed root) under the attempt-scoped
    // scratch namespace, and snapshots off the peer's snapshot lane.
    let code = crate::blob_fetch::FetchingCodeSource::new(
        blobs.clone(),
        client.clone(),
        crate::constants::MAX_MODULE_CODE_BYTES,
        crate::constants::BLOB_FETCH_ATTEMPTS,
    );
    let mut stores =
        |module: &'static str| -> BoxFut<'_, Result<Box<dyn sdk::MerkleStore>, String>> {
            let child = scratch_context.child(child_label(module));
            let target = fetch_target(module);
            Box::pin(async move {
                let (target, resolver) = target.await?;
                let store = QmdbStore::sync_from(child, module, target, resolver).await?;
                Ok(Box::new(store) as Box<dyn sdk::MerkleStore>)
            })
        };
    let mut snapshots = |module: &'static str| -> BoxFut<'_, Result<Snapshot, String>> {
        if !rides_the_sync_snapshot_lane(module) {
            return Box::pin(async { Ok(None) });
        }
        let fetch = snapshot_of(module);
        Box::pin(async move { fetch.await.map(Some) })
    };
    let net = NetworkBindings {
        invite: &[],
        identity_chain_id: "",
    };
    let bindings = bindings(&net, &[], genesis);
    let modules = compose(
        PRODUCTION,
        &code,
        &mut stores,
        &disk_substrates(forge_repo, files_scratch.dir(), blobs.clone()),
        &bindings,
        Boot::Reopen {
            snapshots: &mut snapshots,
        },
    )
    .await?;
    // compose and check THE property: the rebuilt root-hash IS the manifest's.
    // the topology keeps this set in lockstep with [`genesis_host`] by
    // construction — a missing module composes a different root-hash and the
    // join fails its final check.
    let mut host = host_from(modules).map_err(|e| format!("compose synced host: {e}"))?;
    adopt_admitted_modules(&mut host, &code, manifest.height, &mut |id| {
        let fetch = snapshot_of(id);
        Box::pin(async move { fetch.await.map(Some) })
    })
    .await?;
    if host.root_hash() != manifest.root_hash {
        return Err(format!(
            "composed {} != manifest {}",
            hex(&host.root_hash()),
            hex(&manifest.root_hash)
        ));
    }
    // the composite gate passed — promote files' scratch into the canonical
    // `duckfs_dir` (verify-then-replace refs + content-addressed object merge,
    // gated on the exact files root this composition certified) and swap the
    // registry onto a canonical-backed module. the returned host must run in
    // place over the canonical dir: the post-reboot full-sync fallback keeps
    // it live without a reboot, and a joiner's promotion reboot re-opens the
    // same dir. on any error the host is discarded and the retry re-syncs —
    // an already-promoted canonical dir is verified state, never damage.
    files_scratch
        .promote(files_root.0)
        .map_err(|e| format!("duckfs promote: {e}"))?;
    host.register(
        compose_module(
            TOPOLOGY.spec("files").expect("files is in the topology"),
            &code,
            &mut stores,
            &disk_substrates(forge_repo, duckfs_dir, blobs.clone()),
            &bindings,
            &mut Boot::Reopen {
                snapshots: &mut snapshots,
            },
        )
        .await
        .map_err(|e| format!("duckfs reopen: {e}"))?,
    );
    // re-check THE property against the canonical-backed composition.
    if host.root_hash() != manifest.root_hash {
        return Err(format!(
            "canonical duckfs reopen composed {} != manifest {}",
            hex(&host.root_hash()),
            hex(&manifest.root_hash)
        ));
    }
    Ok(host)
}

#[cfg(test)]
mod tests {
    use commonware_runtime::Runner as _;

    use super::*;
    use crate::constants::MODULE_IDS;

    /// The PRODUCTION genesis root hash over [`PIN_BINDINGS`] and an EMPTY
    /// validator set — the consensus root every node of such a network computes
    /// at block zero, pinned so that moving it is a decision instead of an
    /// accident. Update it ONLY as the deliberate half of a flag day (see
    /// [`production_genesis_root_hash_is_pinned`]).
    const GENESIS_ROOT_HASH: &str =
        "cc4bdf4e5553f501d396a6dff07ebc178b347b4f160cf7e10fd5415a3367f99c";

    /// The bindings [`GENESIS_ROOT_HASH`] is taken over. They are constants
    /// because they are NOT: each rides its module's genesis `__config`
    /// record (the composer's `seed_store_config` for a store-backed tenant,
    /// the `initial_state` install for a Map-backed one), so a real
    /// network's invite namespace and chain id put it on its own root by
    /// design. Pinning a hash only says anything against fixed ones.
    const PIN_BINDINGS: NetworkBindings<'static> = NetworkBindings {
        invite: b"parity-test",
        identity_chain_id: "parity-test",
    };

    /// Compose the production genesis host in a throwaway storage root and
    /// return `(module ids sorted, root hash hex, native module ids sorted)` —
    /// everything all three pins below need, so none has to keep its own copy
    /// of the construction.
    ///
    /// Production runs this root future on macOS's ~8 MiB process stack, and
    /// the reason to run the test twin on the same budget is to MATCH
    /// production, not to clear a measured bar: libtest's 2 MiB worker stack
    /// was observed too small for the full composition's debug build back when
    /// the set was 20 modules, and dropping to 19 was never re-measured.
    const GENESIS_TEST_STACK_BYTES: usize = 8 * 1024 * 1024;

    /// the genesis code set the pins compose over: the founding set the build
    /// staged beside this test executable — the committed components (the
    /// kernel fixtures pin the same bytes), read and hashed at test time,
    /// never embedded.
    fn fixture_genesis() -> GenesisModules {
        let dir = workspace_config::modules_dir().expect("the build stages the founding set");
        let hashes = crate::config::hash_bundle(&dir, &TOPOLOGY.wasm_ids(PRODUCTION))
            .expect("founding set");
        GenesisModules {
            hashes,
            source: GenesisSource::FoundingSet(dir),
        }
    }

    /// a bare index store in `dir` for the genesis's guests to converge into.
    fn test_index(dir: &std::path::Path) -> indexer::IndexStore {
        indexer::IndexStore::open_bare(dir.join("index"), MODULE_IDS).expect("open index")
    }

    fn genesis_facts() -> (Vec<String>, String, Vec<String>) {
        std::thread::Builder::new()
            .name("production-genesis-test".into())
            .stack_size(GENESIS_TEST_STACK_BYTES)
            .spawn(compose_genesis_facts)
            .expect("spawn production genesis test")
            .join()
            // Keep the original assertion or construction panic and location;
            // turning it into a generic join failure would hide the regression.
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload))
    }

    fn compose_genesis_facts() -> (Vec<String>, String, Vec<String>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let forge_repo = dir.path().join("forge");
        let duckfs_dir = dir.path().join("duckfs");
        let cfg = commonware_runtime::tokio::Config::default()
            .with_storage_directory(dir.path().join("storage"));
        let executor = commonware_runtime::tokio::Runner::new(cfg);
        let index = test_index(dir.path());
        executor.start(|context| async move {
            let host = genesis_host(
                &context,
                &[],
                PIN_BINDINGS,
                NodeSubstrates {
                    forge_repo: &forge_repo,
                    duckfs_dir: &duckfs_dir,
                    blobs: blobstore::BlobHandle::default(),
                    index: &index,
                },
                &fixture_genesis(),
            )
            .await
            .expect("genesis host");
            // module_roots iterates the host's BTreeMap — sorted by id.
            let ids: Vec<String> = host.module_roots().into_iter().map(|(id, _)| id).collect();
            // a module with no code hash is one the binary compiled in rather
            // than one the registry can swap; `ids` is already sorted, so this is.
            let native = ids
                .iter()
                .filter(|id| host.module_code_hash(id).is_none())
                .cloned()
                .collect();
            (ids, hex(&host.root_hash()), native)
        })
    }

    /// a module the modules registry admitted AFTER the last checkpoint has
    /// no snapshot there: restore registers it empty for replay to rebuild
    /// (an `Err` here would refuse every restart between an activation and
    /// the next checkpoint). a captured module still restores its bytes.
    #[test]
    fn an_admission_after_the_checkpoint_restores_empty() {
        std::thread::Builder::new()
            .name("admission-after-checkpoint-test".into())
            .stack_size(GENESIS_TEST_STACK_BYTES)
            .spawn(|| {
                let dir = tempfile::tempdir().expect("tempdir");
                let cfg = commonware_runtime::tokio::Config::default()
                    .with_storage_directory(dir.path().join("storage"));
                let executor = commonware_runtime::tokio::Runner::new(cfg);
                let index = test_index(dir.path());
                executor.start(|context| async move {
                    let host = genesis_host(
                        &context,
                        &[],
                        PIN_BINDINGS,
                        NodeSubstrates {
                            forge_repo: &dir.path().join("forge"),
                            duckfs_dir: &dir.path().join("duckfs"),
                            blobs: blobstore::BlobHandle::default(),
                            index: &index,
                        },
                        &fixture_genesis(),
                    )
                    .await
                    .expect("genesis host");
                    let manifest =
                        Manifest::capture(&host, None, 0, 0, Vec::new(), Vec::new(), None, 0, 1)
                            .expect("capture");
                    let captured = admitted_restore_snapshot(&manifest, "runs").expect("runs");
                    assert!(
                        captured.is_some(),
                        "a captured Map tenant restores its bytes"
                    );
                    let later = admitted_restore_snapshot(&manifest, "admitted-later")
                        .expect("an uncaptured admission is not an error");
                    assert!(later.is_none(), "it registers empty for replay");
                })
            })
            .expect("spawn")
            .join()
            .unwrap_or_else(|payload| std::panic::resume_unwind(payload));
    }

    /// a modules registry that activated `hello` on `first` at 10 and on
    /// `second` at 50 — the shape a restart finds after a live swap.
    fn registry_ahead(first: [u8; 32], second: [u8; 32]) -> modules::Modules {
        use modules::{Modules, ModulesMsg, encode_msg};
        use sdk::{Module as _, Origin};
        let member = vec![7u8; 32];
        let one_member = {
            let member = member.clone();
            move |req: &[u8]| -> Result<Vec<u8>, sdk::Error> {
                match valset::decode_query(req) {
                    Ok(valset::ValsetQuery::Validators) => Ok(valset::encode_reply(
                        &valset::ValsetReply::Validators(vec![member.clone()]),
                    )),
                    _ => Err(sdk::Error::QueryUnsupported),
                }
            }
        };
        let ctx = |origin: Origin, height: u64| {
            sdk_testkit::TestCtx::with_env(sdk::Env {
                height,
                consensus_time: 0,
                origin,
                me: "modules".into(),
            })
            .on_query("valset", one_member.clone())
        };
        let msg = |m: ModulesMsg| sdk::Msg {
            target: "modules".into(),
            payload: encode_msg(&m),
        };
        let steps = [
            (
                Origin::System,
                10,
                ModulesMsg::RegisterModule {
                    module_id: "hello".into(),
                    code_hash: first.to_vec(),
                },
            ),
            (
                Origin::System,
                11,
                ModulesMsg::ScheduleSwap {
                    name: "next".into(),
                    module_id: "hello".into(),
                    activation_height: 50,
                    code_hash: second.to_vec(),
                },
            ),
            (
                Origin::External(member),
                12,
                ModulesMsg::SwapReady {
                    name: "next".into(),
                    module_id: "hello".into(),
                },
            ),
            (Origin::System, 50, ModulesMsg::Advance),
        ];
        let mut registry = Modules::new(
            "modules",
            Box::new(sdk_testkit::MemStore::new()),
            "valset",
        );
        futures::executor::block_on(async {
            for (origin, height, m) in steps {
                let mut ctx = ctx(origin, height);
                registry
                    .execute(&mut ctx, &msg(m))
                    .await
                    .expect("registry op");
                registry.commit_block().await.expect("commit");
            }
        });
        registry
    }

    /// the registry reopens AHEAD of the checkpoint: a module it swapped after
    /// the checkpoint is adopted on the code that sealed the checkpoint's
    /// block, not the tip's active hash — replay's realization moves it
    /// forward from there. a checkpoint before the module's first activation
    /// seats that first code.
    #[test]
    fn adoption_seats_the_code_at_the_checkpoint_height() {
        const HELLO_V1: &[u8] =
            include_bytes!("../../../crates/kernel/host/tests/fixtures/hello.component.wasm");
        const HELLO_REPLACEMENT: &[u8] = include_bytes!(
            "../../../crates/kernel/host/tests/fixtures/hello-replacement.component.wasm"
        );
        let blobs = blobstore::BlobHandle::default();
        let v1 = blobs.put_chunk(HELLO_V1.to_vec());
        let replacement = blobs.put_chunk(HELLO_REPLACEMENT.to_vec());
        let code = BlobCodeSource(std::sync::Arc::new(blobs));
        for (checkpoint_height, want) in [(5, v1), (20, v1), (50, replacement), (70, replacement)] {
            let mut host = Host::new();
            host.register(Box::new(registry_ahead(v1, replacement)));
            futures::executor::block_on(adopt_admitted_modules(
                &mut host,
                &code,
                checkpoint_height,
                &mut |_| Box::pin(async { Ok(None) }),
            ))
            .expect("adopt");
            assert_eq!(
                host.module_code_hash("hello"),
                Some(want.to_vec()),
                "checkpoint at {checkpoint_height}"
            );
        }
    }

    /// the registry ↔ topology parity pin. the composer already builds
    /// genesis, restore, and state sync from the one `PRODUCTION` selection;
    /// this test pins that set to `MODULE_IDS` — the same selection the
    /// status/index surfaces iterate — so adding a module to one but not the
    /// other fails here instead of silently misreporting.
    #[test]
    fn genesis_registry_matches_module_ids() {
        let (got, _root, _native) = genesis_facts();
        let mut want: Vec<String> = MODULE_IDS.iter().map(|s| s.to_string()).collect();
        want.sort_unstable();
        assert_eq!(got, want);
    }

    /// the topology's `code` column is what the loader branches on; if it
    /// disagrees with what the composed host actually runs, a native module is
    /// sent to the wasm loader (or a wasm tenant is never reconciled).
    #[test]
    fn topology_code_column_matches_the_composed_host() {
        let in_production_and_native = |m: &topology::ModuleSpec| {
            MODULE_IDS.contains(&m.id) && m.code == topology::Code::Native
        };
        let mut native_by_topology: Vec<String> = topology::TOPOLOGY
            .modules
            .iter()
            .filter(|m| in_production_and_native(m))
            .map(|m| m.id.to_string())
            .collect();
        native_by_topology.sort_unstable();
        let (_ids, _root, native_by_host) = genesis_facts();
        assert_eq!(native_by_host, native_by_topology);
        // both sides go empty together if the last native module ever leaves
        // PRODUCTION, so anchor the pin on the ids themselves as well.
        assert_eq!(native_by_host, ["modules", "valset"]);
    }

    /// a workspace genesis seeds every component into the blob store and
    /// itself as a chunk keyed by its pin; an absent file is REPORTED (a
    /// joiner fetches it) unless the blob store holds the chunk, which
    /// rewrites the file; a tampered file refuses by the whole-file hash; a
    /// file whose component disagrees with the descriptor refuses by module
    /// id.
    #[test]
    fn a_workspace_genesis_seeds_and_serves_itself() {
        let dir = tempfile::tempdir().expect("tempdir");
        let genesis = Genesis {
            components: vec![workspace_config::Artifact {
                id: "pages".into(),
                bytes: b"pages-bytes".to_vec(),
            }],
            index_guests: vec![],
        };
        let bytes = genesis.encode();
        let hash = workspace_config::genesis::sha256(&bytes);
        let want = genesis.component_hashes();
        let file = workspace_config::genesis_path(dir.path());

        let blobs = blobstore::BlobHandle::default();
        assert!(
            seed_workspace_genesis(&blobs, &file, &hash, &want)
                .expect("absence is reported")
                .is_none()
        );
        std::fs::write(&file, &bytes).expect("write");
        let loaded = seed_workspace_genesis(&blobs, &file, &hash, &want)
            .expect("seed")
            .expect("present");
        assert_eq!(loaded, genesis);
        assert!(blobs.has_chunk(&want["pages"]), "the component");
        assert!(blobs.has_chunk(&hash), "the genesis itself, for the next joiner");

        // the file is the chunk's readable copy: lose it, and the seeded
        // store writes it back.
        std::fs::remove_file(&file).expect("remove");
        let rewritten = seed_workspace_genesis(&blobs, &file, &hash, &want)
            .expect("seed")
            .expect("rewritten from the blob store");
        assert_eq!(rewritten, genesis);
        assert_eq!(std::fs::read(&file).expect("the file is back"), bytes);

        let mut tampered = bytes.clone();
        tampered.push(0);
        std::fs::write(&file, &tampered).expect("write");
        let err = seed_workspace_genesis(&blobstore::BlobHandle::default(), &file, &hash, &want)
            .unwrap_err();
        assert!(err.contains("not the network's genesis"), "{err}");

        std::fs::write(&file, &bytes).expect("write");
        let mut wrong = want.clone();
        wrong.insert("pages".into(), [0u8; 32]);
        let err = seed_workspace_genesis(&blobstore::BlobHandle::default(), &file, &hash, &wrong)
            .unwrap_err();
        assert!(err.contains("module pages"), "{err}");
    }

    /// the dev shape's founding set: every file must hash to the descriptor's
    /// entry and land in the blob store; a mismatch or an absence refuses by
    /// module id and path.
    #[test]
    fn a_founding_set_seeds_every_component_or_refuses_by_name() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("pages.component.wasm"), b"pages-bytes").expect("write");
        let mut want = std::collections::BTreeMap::new();
        want.insert(
            "pages".to_string(),
            sha2::Sha256::digest(b"pages-bytes").into(),
        );
        let blobs = blobstore::BlobHandle::default();
        seed_founding_set(&blobs, dir.path(), &want).expect("seed");
        assert!(blobs.has_chunk(&want["pages"]));

        std::fs::write(dir.path().join("pages.component.wasm"), b"tampered").expect("write");
        let err = seed_founding_set(&blobstore::BlobHandle::default(), dir.path(), &want)
            .unwrap_err();
        assert!(err.contains("module pages"), "{err}");

        std::fs::remove_file(dir.path().join("pages.component.wasm")).expect("remove");
        let err = seed_founding_set(&blobstore::BlobHandle::default(), dir.path(), &want)
            .unwrap_err();
        assert!(err.contains("pages.component.wasm"), "{err}");
        // a chunk the store already holds needs no file at all.
        seed_founding_set(&blobs, dir.path(), &want).expect("seeded store");
    }

    /// THE consensus pin: the production genesis root hash is a constant.
    ///
    /// It is the only ABSOLUTE one in the tree, and until it existed every claim
    /// that "the root hash did not move" was relative and therefore weak.
    /// `bin/simnode/tests/topology_set.rs` pins the 15-module sim composition —
    /// which excludes `acl`, `governance`, `modules` and `valset`, and is not
    /// what a node runs. (Not a NATIVE composition, as this said for a while:
    /// simnode opens a `DirCodeSource` over the founding set the build staged
    /// beside it and composes through `noded::compose`, so every `SIM_BASE`
    /// id loads as a wasm component — which is why a rebuilt component moves
    /// that root.) And `git
    /// diff crates/modules/` on a committed tree is EMPTY BY CONSTRUCTION, so
    /// quoting it proves nothing at all. Neither would have noticed a module's
    /// bytes changing.
    ///
    /// ## the mechanism, because it surprises everyone once
    ///
    /// What this covers is wider than the module SET. The composer's modules registry
    /// seed commits `sha256(component.wasm)` — the descriptor's hash — for
    /// every wasm tenant into the modules registry's MerkleStore, so each
    /// guest's CODE DIGEST is consensus state itself. That means a module's
    /// SOURCE is consensus-relevant the moment its component is rebuilt — even
    /// for a change that alters no behaviour, even a comment — and it means
    /// `make wasm-modules` can ship a seventeen-module flag day as a side effect
    /// of touching one guest. That is correct, and it is exactly the event that
    /// must never happen silently.
    ///
    /// ## when this fails
    ///
    /// You are in one of two situations and the message says so, because they
    /// need opposite responses:
    ///
    /// - **On purpose.** A module was added or removed, a guest was rebuilt, a
    ///   genesis-seeded record changed. Then this hash SHOULD move: update the
    ///   constant in the same commit, and say in the commit message which change
    ///   moved it. A flag day is cheap — there is no live chain — but it has to
    ///   be a stated act.
    /// - **By accident.** You did not mean to touch consensus, and you did. The
    ///   usual cause is a rebuilt `component.wasm` riding along in the diff.
    #[test]
    fn production_genesis_root_hash_is_pinned() {
        let (_ids, root, _native) = genesis_facts();
        assert_eq!(
            root, GENESIS_ROOT_HASH,
            "the production genesis root hash MOVED.\n\
             Every node computes this at block zero, so a network whose members \
             do not all agree on it forks at genesis.\n\
             \n\
             DID YOU MEAN TO? A module added/removed, a guest rebuilt, a \
             genesis-seeded record changed — then yes, and this is a deliberate \
             flag day: set GENESIS_ROOT_HASH to {root} in the SAME commit as the \
             change that moved it, and name that change in the commit message.\n\
             \n\
             DID YOU NOT? Then you have moved consensus by accident. Look for a \
             rebuilt component.wasm in your diff — a guest's code digest is \
             consensus state, so `make wasm-modules` moves this hash even when \
             the source change was cosmetic:\n\
             \x20 git diff origin/dev --name-only crates/modules/ crates/guests/ crates/examples/"
        );
    }
}
