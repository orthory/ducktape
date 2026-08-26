//! Application-host construction, restoration, and state synchronization.
//!
//! This module owns the module registry and the durable swap from scratch
//! state into the canonical host. The live node loop only consumes the three
//! lifecycle operations and output adapter exported below.

use commonware_cryptography::ed25519;
use commonware_runtime::Supervisor as _;
use duckfs_disk::SyncScratch;
use files::{Files, FilesOdbBacking};
use forge::Forge;
use host::Host;
use lifecycle::Lifecycle;
use recovery::Manifest;
use sdk::StateRoot;
use sha2::Digest as _;
use statesync::{
    fetch_snapshot,
    qmdb::{QmdbStore, RemoteQmdbResolver},
};
use valset::Valset;
use wasm_host::WasmModule;

use crate::util::hex;

/// Consensus-visible network names shared by genesis, restore, and state sync.
#[derive(Clone, Copy)]
pub(super) struct NetworkBindings<'a> {
    pub(super) invite: &'a [u8],
    pub(super) identity_chain_id: &'a str,
}

/// Node-local substrates used only while reconstructing a host from state sync.
pub(super) struct SyncSubstrates<'a> {
    pub(super) forge_repo: &'a std::path::Path,
    pub(super) duckfs_dir: &'a std::path::Path,
    pub(super) blobs: blobstore::BlobHandle,
}

/// the reference wasm module's GENESIS component — embedded so every node
/// constructs the identical module from the identical bytes (its sha256 is the
/// genesis-seeded active hash in the code registry). live updates arrive
/// out-of-band as governance-committed hashes + blob-plane bytes; this is only
/// where the story starts.
const HELLO_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/guests/hello-wasm/component.wasm");

/// the genesis-constant id the reference wasm module registers under.
const HELLO_WASM_MODULE_ID: &str = "hello";

/// the blobstore-backed [`host::CodeSource`]: component bytes for a code swap
/// are content-addressed chunks on the node's blob plane (staged there before
/// the governance schedule, exactly like a forge Push packfile). a hash the
/// store lacks is a `None` — the boundary fails closed rather than forking.
pub(super) struct BlobCodeSource(pub(super) std::sync::Arc<dyn blobstore::Blobs>);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for BlobCodeSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        self.0.get_chunk(&digest)
    }
}

/// the wasm-runtime [`host::ModuleFactory`]: a post-genesis ADMISSION
/// (governance `RegisterModule` → lifecycle `ScheduleRegister`) instantiates its
/// module from the verified component bytes at the activation boundary — the
/// constructor twin of [`BlobCodeSource`].
pub(super) struct WasmModuleFactory;

impl host::ModuleFactory for WasmModuleFactory {
    fn instantiate(&self, id: &str, bytes: &[u8]) -> Result<Box<dyn sdk::Module>, sdk::Error> {
        Ok(Box::new(WasmModule::from_bytes(id, bytes)?))
    }
}

/// the directory module's GENESIS component.
const DIRECTORY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/directory/component.wasm");

/// the genesis-constant id the directory module registers under.
const DIRECTORY_MODULE_ID: &str = "directory";

/// inbox — a STORE-BACKED tenant like pages/chat: per-member queues ride a
/// host-constructed qmdb store (nothing enumerates members; the read surface
/// is the index tier). tasks is STORE-BACKED the same way: the `tasks` GENESIS
/// component hosts the task and job boards over one host-constructed qmdb
/// store, one record per task/job, so capture is O(1) and an op costs only the
/// keys it touches.
const INBOX_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/inbox/component.wasm");
const INBOX_MODULE_ID: &str = "inbox";
const TASKS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/tasks/component.wasm");
const TASKS_MODULE_ID: &str = "tasks";

/// tagging — a STORE-BACKED tenant like pages/chat: the subscription plane
/// rides a host-constructed qmdb store (pure point records — nothing
/// enumerates scopes) and the wasm root is the store's merkle root. the
/// lifecycle module deliberately stays NATIVE:
/// its Advance decides over frozen end-of-block committed state, a surface the
/// wit world's staged-over-committed reads cannot represent (kernel
/// coordinators — valset, lifecycle — gate the machinery itself).
const TAGGING_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/tagging/component.wasm");
const TAGGING_MODULE_ID: &str = "tagging";
/// acl — a STORE-BACKED tenant like tagging/capability: the submit-policy
/// table rides a host-constructed qmdb store and the wasm root is the store's
/// merkle root. the kernel drain's standing gate consults it per external op
/// through the ordinary module query lane, which the guest serves
/// staged-over-committed exactly as the native module did.
const ACL_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/acl/component.wasm");
const ACL_MODULE_ID: &str = "acl";
/// capability — a STORE-BACKED tenant like pages/chat: the provider registry
/// rides a host-constructed qmdb store and the wasm root is the store's
/// merkle root. no per-network config (the valset sibling id is compiled into
/// the guest).
const CAPABILITY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/capability/component.wasm");
const CAPABILITY_MODULE_ID: &str = "capability";

/// identity — a STORE-BACKED tenant like pages/chat/agent/automations, with
/// the governance seam: its per-network chain id travels as GENESIS CONFIG
/// seeded INTO the qmdb store ([`seed_store_config`]) instead of an installed
/// host-KV snapshot, so the config is an ordinary store record in the merkle
/// root from genesis and rides state-sync like every other record.
const IDENTITY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/identity/component.wasm");
const IDENTITY_MODULE_ID: &str = "identity";
/// gateway — a STORE-BACKED tenant with the governance/identity seam: its
/// per-network chain id travels as GENESIS CONFIG seeded INTO the qmdb store
/// ([`seed_store_config`]), an ordinary store record in the merkle root from
/// genesis that rides state-sync like every other record.
const GATEWAY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/gateway/component.wasm");
const GATEWAY_MODULE_ID: &str = "gateway";
/// governance — a STORE-BACKED tenant like pages/chat/agent/automations,
/// with one extra seam: its per-network invite binding travels as GENESIS
/// CONFIG seeded INTO the qmdb store ([`seed_store_config`]) instead of an
/// installed host-KV snapshot, so the config is an ordinary store record in
/// the merkle root from genesis and rides state-sync like every other record.
const GOVERNANCE_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/governance/component.wasm");
const GOVERNANCE_MODULE_ID: &str = "governance";

/// saga — an adapter-ported tenant of the async engine's deterministic half.
/// every decision in its execute paths reads staged-over-committed state, so
/// the whole-state fold is behavior-identical (pinned by its parity proof);
/// its work-order events and P6 callbacks cross the wit seam unchanged.
const SAGA_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/saga/component.wasm");
const SAGA_MODULE_ID: &str = "saga";
/// agent / automations — STORE-BACKED tenants like pages/chat: each rides a
/// host-constructed qmdb store and the wasm root is the store's merkle root.
/// automations' chat-hook probe reads are host-routed `query-module` reads
/// against the live siblings, exactly as when it was snapshot-ported.
const AGENT_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/agent/component.wasm");
const AGENT_MODULE_ID: &str = "agent";
const AUTOMATIONS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/automations/component.wasm");
const AUTOMATIONS_MODULE_ID: &str = "automations";

/// dispatch — the task plane's recipe-manifest + capability-routed delivery
/// registry, adapter-ported like its saga collaborator (the native crate
/// compiled into the guest) and STORE-BACKED like pages/chat/tasks: one record
/// per recipe, per dispatch and per mailbox entry over the host-constructed
/// qmdb store, so a dispatch record — `runs`' PERMANENT turn claim — costs
/// nothing per op and capture is O(1). the wasm root IS the store's root, so
/// the port is ROOT-CONTINUOUS with the native module.
///
/// its query surface is COMMITTED-ONLY regardless of caller — the host's
/// `PendingDeliveries` delivery injection and runs' `turn_taken` existence read
/// must never see a same-block staged write — pinned by the genesis builder's
/// `.with_committed_queries()`, which drops the outer staged overlay for a query
/// round so `WitStore` serves the native module's `get_committed` reads exactly
/// as the native store does. dispatch carries NO ctx-routed enrichment: the
/// former `query_with` assignee facade was retired when runs' `lease_holder`
/// moved onto saga, so the guest's ctx-less query is exactly faithful.
///
/// its saga collaborator id ("saga") is genesis-constant, compiled into the guest.
const DISPATCH_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/system/dispatch/component.wasm");
const DISPATCH_MODULE_ID: &str = "dispatch";

/// runs — the FINAL adapter-ported tenant: the collaboration loop's actor.
/// every decision in its handle paths reads staged-over-committed (the
/// watch/pending-entry/session accessors shadow the committed maps with the
/// block's overlays), so the whole-state fold is behavior-identical (pinned
/// by `wasm_runs_parity`). its ten collaborator ids — chat, saga, tagging,
/// dispatch, agent, tasks, plus the files/forge/pages builder chain —
/// are genesis-constant and compiled into the guest (the exact production
/// constructor these builders used to call natively). its two dispatch reads —
/// `turn_taken`'s existence check and `lease_holder`'s `AwaitingResult { saga_id }`
/// lookup — both read COMMITTED-ONLY dispatch fields, served faithfully by
/// dispatch's own guest query lane (`with_committed_queries`), so the host-routed
/// query reads exactly the committed record, never a same-block staged write.
/// only the ASSIGNEE source moved off dispatch: `lease_holder` still reads the
/// saga id FROM the dispatch view, then reads the live lease from saga directly
/// (dispatch's retired `query_with` no longer relays it). the
/// delivered-runs ring (`RecentRuns`) — derived per-node state outside the
/// NATIVE root/snapshot — persists through the guest's own `__history` key
/// (the app's runs client and the dogfood receipt lane read it), so it rides
/// the wasm root and snapshots like everything else the guest keeps.
const RUNS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/runs/component.wasm");
const RUNS_MODULE_ID: &str = "runs";

/// pages / chat are STORE-BACKED wasm tenants. The module root is the
/// host-side qmdb store's Merkle root; tagging wiring lives in the guests.
const PAGES_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/pages/component.wasm");
const PAGES_MODULE_ID: &str = "pages";
const CHAT_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/chat/component.wasm");
const CHAT_MODULE_ID: &str = "chat";

/// files (duckfs): the guest runs pure
/// `duckfs-core` over the WIT object plane while the HOST keeps the disk odb +
/// durable refs file behind a [`FilesOdbBacking`] (`WasmModule::with_odb`).
/// Its module root is `sha256(encode_refs)`.
const FILES_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/modules/apps/files/component.wasm");
const FILES_MODULE_ID: &str = "files";

/// genesis-seed the code registry: every wasm tenant's initial active code
/// hash, identical on every node (the embedded components ARE the hashes'
/// preimages). shared by the genesis / restore / state-sync host builders so
/// all three compose the same registry shape.
async fn seeded_lifecycle(store: Box<dyn sdk::MerkleStore>) -> Lifecycle {
    let mut reg = Lifecycle::new(host::LIFECYCLE_MODULE_ID, store, "valset");
    reg.seed(
        HELLO_WASM_MODULE_ID,
        sha2::Sha256::digest(HELLO_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        DIRECTORY_MODULE_ID,
        sha2::Sha256::digest(DIRECTORY_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        INBOX_MODULE_ID,
        sha2::Sha256::digest(INBOX_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        TASKS_MODULE_ID,
        sha2::Sha256::digest(TASKS_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        TAGGING_MODULE_ID,
        sha2::Sha256::digest(TAGGING_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        CAPABILITY_MODULE_ID,
        sha2::Sha256::digest(CAPABILITY_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        IDENTITY_MODULE_ID,
        sha2::Sha256::digest(IDENTITY_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        GATEWAY_MODULE_ID,
        sha2::Sha256::digest(GATEWAY_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        GOVERNANCE_MODULE_ID,
        sha2::Sha256::digest(GOVERNANCE_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        PAGES_MODULE_ID,
        sha2::Sha256::digest(PAGES_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        CHAT_MODULE_ID,
        sha2::Sha256::digest(CHAT_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        SAGA_MODULE_ID,
        sha2::Sha256::digest(SAGA_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        AGENT_MODULE_ID,
        sha2::Sha256::digest(AGENT_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        AUTOMATIONS_MODULE_ID,
        sha2::Sha256::digest(AUTOMATIONS_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        RUNS_MODULE_ID,
        sha2::Sha256::digest(RUNS_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        DISPATCH_MODULE_ID,
        sha2::Sha256::digest(DISPATCH_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        FILES_MODULE_ID,
        sha2::Sha256::digest(FILES_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.seed(
        ACL_MODULE_ID,
        sha2::Sha256::digest(ACL_WASM_COMPONENT).to_vec(),
    )
    .await
    .expect("genesis lifecycle seed stages");
    reg.finish_seed()
        .await
        .expect("genesis lifecycle seeds commit");
    reg
}

/// seed the blob plane with the genesis components, so this node can serve
/// (and re-fetch) every wasm tenant's initial code by content hash. runs on
/// EVERY boot path — genesis, restore, state-sync — because a node's binary
/// may embed components the committed registry has moved past (or ahead of):
/// re-putting is idempotent, and having every version this binary knows in
/// the store is what lets the boot reconciliation and the mesh fetch lane
/// close a version skew instead of failing closed on it.
pub(super) fn seed_genesis_components(blobs: &blobstore::BlobHandle) {
    blobs.put_chunk(HELLO_WASM_COMPONENT.to_vec());
    blobs.put_chunk(DIRECTORY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(INBOX_WASM_COMPONENT.to_vec());
    blobs.put_chunk(TASKS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(TAGGING_WASM_COMPONENT.to_vec());
    blobs.put_chunk(CAPABILITY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(IDENTITY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(GATEWAY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(GOVERNANCE_WASM_COMPONENT.to_vec());
    blobs.put_chunk(PAGES_WASM_COMPONENT.to_vec());
    blobs.put_chunk(CHAT_WASM_COMPONENT.to_vec());
    blobs.put_chunk(SAGA_WASM_COMPONENT.to_vec());
    blobs.put_chunk(AGENT_WASM_COMPONENT.to_vec());
    blobs.put_chunk(AUTOMATIONS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(RUNS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(DISPATCH_WASM_COMPONENT.to_vec());
    blobs.put_chunk(FILES_WASM_COMPONENT.to_vec());
    blobs.put_chunk(ACL_WASM_COMPONENT.to_vec());
}

/// the reference wasm module at its GENESIS code. restarted/synced nodes still
/// construct from the embedded bytes — the boot-time code reconciliation
/// (`Host::realize_module_swaps`) swaps to the committed active component when
/// the registry has moved past genesis.
fn genesis_hello_wasm() -> WasmModule {
    WasmModule::from_bytes(HELLO_WASM_MODULE_ID, HELLO_WASM_COMPONENT)
        .expect("embedded hello component loads")
}

/// the directory module at its GENESIS code (same reconciliation story as
/// [`genesis_hello_wasm`]).
fn genesis_directory_wasm() -> WasmModule {
    WasmModule::from_bytes(DIRECTORY_MODULE_ID, DIRECTORY_WASM_COMPONENT)
        .expect("embedded directory component loads")
}

/// inbox at its GENESIS code over the host-constructed store (same three
/// store lifecycles as [`pages_wasm`]).
fn inbox_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(INBOX_MODULE_ID, INBOX_WASM_COMPONENT, store)
        .expect("embedded inbox component loads")
}

/// the `tasks` work module (the task and job boards) at its GENESIS code over
/// the host-constructed store (same three store lifecycles as [`pages_wasm`]).
fn tasks_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(TASKS_MODULE_ID, TASKS_WASM_COMPONENT, store)
        .expect("embedded tasks component loads")
}

/// tagging at its GENESIS code over the host-constructed store (same three
/// store lifecycles as [`pages_wasm`]).
fn tagging_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(TAGGING_MODULE_ID, TAGGING_WASM_COMPONENT, store)
        .expect("embedded tagging component loads")
}

/// capability at its GENESIS code over the host-constructed store (same
/// three store lifecycles as [`pages_wasm`]).
fn capability_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(CAPABILITY_MODULE_ID, CAPABILITY_WASM_COMPONENT, store)
        .expect("embedded capability component loads")
}

/// acl at its GENESIS code over the host-constructed store (same three store
/// lifecycles as [`pages_wasm`]). deliberately EMPTY at genesis: an empty
/// policy table is allow-all, and only governance follow-ups tighten it.
fn acl_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(ACL_MODULE_ID, ACL_WASM_COMPONENT, store)
        .expect("embedded acl component loads")
}

/// saga at its GENESIS code (adapter-ported) over the host-constructed store.
/// the sibling wiring — saga's valset/capability assignment reads — is
/// genesis-constant and compiled into the guest (the exact production
/// constructor these builders used to call natively). agent's wiring (saga
/// dead-letter + runs hook) and automations' (the chat/tasks/inbox lanes) are
/// compiled into their guests the same way; only their stores arrive
/// host-constructed ([`agent_wasm`], [`automations_wasm`]).
fn saga_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(SAGA_MODULE_ID, SAGA_WASM_COMPONENT, store)
        .expect("embedded saga component loads")
}

/// agent at its GENESIS code over the host-constructed store (see
/// [`pages_wasm`] for the three store lifecycles — init, reopen, sync_from —
/// which all hand the store in the same way).
fn agent_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(AGENT_MODULE_ID, AGENT_WASM_COMPONENT, store)
        .expect("embedded agent component loads")
}

/// automations at its GENESIS code over the host-constructed store (same
/// three store lifecycles as [`pages_wasm`]).
fn automations_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(AUTOMATIONS_MODULE_ID, AUTOMATIONS_WASM_COMPONENT, store)
        .expect("embedded automations component loads")
}

/// dispatch at its GENESIS code over the host-constructed store (same three
/// store lifecycles as [`pages_wasm`]). `.with_committed_queries()` pins the
/// guest's query lane committed-only, preserving the native read facade's
/// contract (the host's delivery injection + runs' `turn_taken` read); EXACTLY
/// the wiring `wasm_dispatch_parity`'s `wasm_dispatch()` pins.
fn dispatch_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(DISPATCH_MODULE_ID, DISPATCH_WASM_COMPONENT, store)
        .expect("embedded dispatch component loads")
        .with_committed_queries()
}

/// runs at its GENESIS code (adapter-ported — see the component const's doc).
/// The native canonical snapshot and delivered-runs ring are persisted in the
/// current v1 host-KV layout.
fn genesis_runs_wasm() -> WasmModule {
    WasmModule::from_bytes(RUNS_MODULE_ID, RUNS_WASM_COMPONENT)
        .expect("embedded runs component loads")
}

/// pages at its GENESIS code over the host-constructed store (a fresh/reopened
/// `QmdbStore::init`, or a `QmdbStore::sync_from` at a verified root — all
/// three lifecycles hand the store in the same way, exactly as they handed it
/// to the module constructor). The committed encoding is the store's op log
/// and the module root is its Merkle root.
fn pages_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(PAGES_MODULE_ID, PAGES_WASM_COMPONENT, store)
        .expect("embedded pages component loads")
}

/// chat at its GENESIS code over the host-constructed store.
fn chat_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(CHAT_MODULE_ID, CHAT_WASM_COMPONENT, store)
        .expect("embedded chat component loads")
}

/// files at its GENESIS code over a host-side duckfs substrate at `dir`.
/// `open` recovers committed refs, durable height, and the GC watermark from
/// the on-disk envelope. `root()` is `sha256(refs_bytes)`.
fn files_wasm(dir: std::path::PathBuf) -> Result<WasmModule, sdk::Error> {
    let backing = FilesOdbBacking::open(FILES_MODULE_ID, dir)?;
    WasmModule::with_odb(FILES_MODULE_ID, FILES_WASM_COMPONENT, Box::new(backing))
}

/// identity at its GENESIS code over the host-constructed store (same three
/// store lifecycles as [`pages_wasm`]). the per-network chain id every signed
/// certificate preimage folds in is a `__config` STORE RECORD, seeded at
/// genesis by [`seed_store_config`]; the submit-door client ACL and both
/// ownership indexes are ordinary store records in the same root.
fn identity_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(IDENTITY_MODULE_ID, IDENTITY_WASM_COMPONENT, store)
        .expect("embedded identity component loads")
}

/// gateway at its GENESIS code over the host-constructed store (same three
/// store lifecycles as [`pages_wasm`]). It owns the `.duck` handle and route
/// planes; both are chain-scoped, so the per-network chain id rides the
/// store-seeded `__config` record ([`seed_store_config`]).
fn gateway_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(GATEWAY_MODULE_ID, GATEWAY_WASM_COMPONENT, store)
        .expect("embedded gateway component loads")
}

/// governance at its GENESIS code over the host-constructed store (same
/// three store lifecycles as [`pages_wasm`]). the invite binding every token
/// and join proof verify against is a `__config` STORE RECORD, seeded at
/// genesis by [`seed_store_config`] (the sibling wiring —
/// valset/lifecycle/identity — is genesis-constant and compiled into the
/// guest like every other port's sibling ids).
fn governance_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(GOVERNANCE_MODULE_ID, GOVERNANCE_WASM_COMPONENT, store)
        .expect("embedded governance component loads")
}

/// seed a STORE-BACKED wasm tenant's genesis config: commit the reserved
/// `__config` record ([`sdk::genesis_config`]) into its qmdb store under
/// [`sdk::store_key`] — the exact slot the module's own `StagedStore` maps
/// that logical key to, where the guest's `load_store_config` reads it back.
/// committed at genesis construction, the config is part of the store's
/// merkle root from block zero (genesis roots honestly differ per network)
/// and rides state-sync like any other record. idempotent: a store that
/// already carries a config (a reopened workspace re-entering the genesis
/// path) is left byte-untouched.
async fn seed_store_config(store: &mut dyn sdk::MerkleStore, params: &[(&str, &[u8])], what: &str) {
    let key = sdk::store_key(sdk::genesis_config::CONFIG_KEY);
    let already = store
        .get(&key)
        .await
        .unwrap_or_else(|e| panic!("{what} genesis config read: {e}"));
    if already.is_some() {
        return;
    }
    let config = sdk::genesis_config::encode_config(params);
    store
        .commit_batch(vec![(key, Some(config))])
        .await
        .unwrap_or_else(|e| panic!("{what} genesis config seeds: {e}"));
}

/// the production module registry: ONE named field per module, so genesis,
/// restore, and state sync compose the SAME set by construction — adding a
/// module means adding a field here, and every lifecycle fails to compile
/// until it builds one. `constants::MODULE_IDS` mirrors this set for the
/// status surfaces; the parity test below pins the two together.
struct ProductionModules {
    pages: WasmModule,
    chat: WasmModule,
    forge: Forge,
    valset: Valset,
    acl: WasmModule,
    governance: WasmModule,
    lifecycle: Lifecycle,
    hello_wasm: WasmModule,
    saga: WasmModule,
    capability: WasmModule,
    dispatch: WasmModule,
    tagging: WasmModule,
    tasks: WasmModule,
    identity: WasmModule,
    gateway: WasmModule,
    inbox: WasmModule,
    files: WasmModule,
    agent: WasmModule,
    runs: WasmModule,
    directory: WasmModule,
    automations: WasmModule,
}

impl ProductionModules {
    /// compose the registry into a [`Host`]. registration order is NOT
    /// consensus-relevant (the host keys modules in a `BTreeMap`) — only the
    /// module set and each module's constructed state compose the root-hash.
    fn compose(self) -> Result<Host, sdk::Error> {
        let mut host = Host::genesis(vec![
            Box::new(self.pages),
            Box::new(self.chat),
            Box::new(self.forge),
            Box::new(self.valset),
            Box::new(self.acl),
            Box::new(self.governance),
            Box::new(self.lifecycle),
            Box::new(self.hello_wasm),
            Box::new(self.saga),
            Box::new(self.capability),
            Box::new(self.dispatch),
            Box::new(self.tagging),
            Box::new(self.tasks),
            Box::new(self.identity),
            Box::new(self.gateway),
            Box::new(self.inbox),
            Box::new(self.files),
            Box::new(self.agent),
            Box::new(self.runs),
            Box::new(self.directory),
            Box::new(self.automations),
        ])?;
        // every production host admits post-genesis modules through the wasm
        // runtime — genesis, restore, and statesync compositions alike.
        host.set_module_factory(Box::new(WasmModuleFactory));
        Ok(host)
    }
}

/// the PRODUCTION module set — genesis state, identical on every node (a
/// different set composes a different root-hash and the network forks at
/// genesis). system infrastructure (valset seeded with the genesis
/// validators, saga) plus every product module. `forge_repo` is this node's
/// on-disk git substrate; wrapper modules run EMBEDDED substrates for now.
pub(super) async fn genesis_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    genesis_validators: &[ed25519::PublicKey],
    bindings: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
) -> Host {
    // pages/chat/agent/automations are STORE-BACKED wasm tenants: the host
    // still constructs the concrete qmdb stores exactly as before — only the
    // executor wrapped around them changed (and the sibling wiring moved
    // into the guests).
    let pages = pages_wasm(Box::new(
        QmdbStore::init(context.child("pages"), "pages").await,
    ));
    let chat = chat_wasm(Box::new(
        QmdbStore::init(context.child("chat"), "chat").await,
    ));
    let agent = agent_wasm(Box::new(
        QmdbStore::init(context.child("agent"), "agent").await,
    ));
    let automations = automations_wasm(Box::new(
        QmdbStore::init(context.child("automations"), "automations").await,
    ));
    // governance and identity are store-backed too; each per-network
    // parameter is seeded into its store as the `__config` record before the
    // wrapper composes (part of the merkle root from block zero).
    let governance = {
        let mut store = QmdbStore::init(context.child("governance"), "governance").await;
        seed_store_config(&mut store, &[("invite", bindings.invite)], "governance").await;
        governance_wasm(Box::new(store))
    };
    let identity = {
        let mut store = QmdbStore::init(context.child("identity"), "identity").await;
        seed_store_config(
            &mut store,
            &[("chain_id", bindings.identity_chain_id.as_bytes())],
            "identity",
        )
        .await;
        identity_wasm(Box::new(store))
    };
    let capability = capability_wasm(Box::new(
        QmdbStore::init(context.child("capability"), "capability").await,
    ));
    let gateway = {
        let mut store = QmdbStore::init(context.child("gateway"), "gateway").await;
        seed_store_config(
            &mut store,
            &[("chain_id", bindings.identity_chain_id.as_bytes())],
            "gateway",
        )
        .await;
        gateway_wasm(Box::new(store))
    };
    let tagging = tagging_wasm(Box::new(
        QmdbStore::init(context.child("tagging"), "tagging").await,
    ));
    let tasks = tasks_wasm(Box::new(
        QmdbStore::init(context.child("tasks"), "tasks").await,
    ));
    let dispatch = dispatch_wasm(Box::new(
        QmdbStore::init(context.child("dispatch"), "dispatch").await,
    ));
    // the lifecycle registry is NATIVE but store-backed the same way; the
    // genesis seed set commits into its store in one idempotent batch.
    let lifecycle = seeded_lifecycle(Box::new(
        QmdbStore::init(context.child("lifecycle"), "lifecycle").await,
    ))
    .await;
    let inbox = inbox_wasm(Box::new(
        QmdbStore::init(context.child("inbox"), "inbox").await,
    ));
    seed_genesis_components(&blobs);
    // forge shares the blob plane so a Push's packfile (staged on the blob
    // lane before submit) can materialize locally; the pack never touches root.
    let forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .expect("forge init")
        .with_chat("chat");
    // the membership registry is NATIVE but store-backed like lifecycle:
    // genesis-seed the validator set from config — deterministic and identical
    // on every node, so membership is IN consensus state from block zero (the
    // substrate epoch cutover + governance will drive) — committed into its
    // store in one idempotent batch.
    let mut valset = Valset::new(
        "valset",
        Box::new(QmdbStore::init(context.child("valset"), "valset").await),
    );
    for v in genesis_validators {
        valset
            .seed(v.as_ref().to_vec())
            .await
            .expect("genesis validator keys are well-formed ed25519");
    }
    valset
        .finish_seed()
        .await
        .expect("genesis valset seed commits");
    // the submit-policy federation is a STORE-BACKED wasm tenant like
    // tagging/capability, and deliberately EMPTY at genesis: an empty table
    // is allow-all, so a fresh network admits any validly signed frame to any
    // module and the table exists only for governance to tighten later.
    let acl_table = acl_wasm(Box::new(
        QmdbStore::init(context.child("acl"), "acl").await,
    ));
    ProductionModules {
        pages,
        chat,
        forge,
        valset,
        acl: acl_table,
        // governance is the SOLE authorized author of valset changes: member
        // proposals + ballots, deterministic tally, follow-up membership ops,
        // and the redeem-time resident grant. store-backed over the
        // host-constructed qmdb store; the invite binding is the store-seeded
        // `__config` record, sibling ids compiled into the guest.
        governance,
        // the network module-code registry: the consensus commitment to WHICH
        // component each hot-swappable wasm module runs, seeded with the genesis
        // hashes. governance schedules height-gated swaps into it; the host
        // realizes them through the blobstore-backed CodeSource. native but
        // store-backed over the host-constructed qmdb store.
        lifecycle,
        // the reference wasm module — the live-update machinery's first tenant.
        hello_wasm: genesis_hello_wasm(),
        // capability-aware strict leases: a saga whose trigger names a
        // capability is assigned over that tag's announced providers, and
        // only the assignee's result lands. an UNASSIGNED attempt (empty
        // provider pool) accepts no result at all: its WorkerRequest is an
        // announcement a capable node must first claim via `SagaMsg::Accept`.
        // adapter-ported; the valset/capability wiring and the Strict policy
        // are compiled into the guest.
        saga: saga_wasm(Box::new(
            QmdbStore::init(context.child("saga"), "saga").await,
        )),
        // the network-wide registry of node host capabilities ("codex",
        // "claude", ...): member-gated self-announcements, so every node holds
        // an identical view of who can run what. store-backed over the
        // host-constructed qmdb store.
        capability,
        // the task plane: recipe manifests + capability-routed dispatch with
        // next-block result delivery (the host's DeliverPending injection).
        // adapter-ported and store-backed over the host-constructed qmdb store;
        // committed-only query lane.
        dispatch,
        // the engagement plane: content modules report tags, subscriber
        // modules receive engagement events — router only, module-agnostic.
        // store-backed over the host-constructed qmdb store.
        tagging,
        // the work plane: the task board (ordered lists) and the job board
        // (first-claim work items). store-backed over the host-constructed
        // qmdb store — one record per task/job, so capture is O(1).
        tasks,
        // the deterministic user->nodes binding registry: certificates are
        // chain-scoped (this network's chain id, riding its store-seeded
        // GENESIS CONFIG), member-gated binds via valset, and account display
        // names have this single canonical owner. store-backed over the
        // host-constructed qmdb store.
        identity,
        // the MERGED gateway: the whole `.duck` name → AccountId → route
        // pipeline in ONE module — the route plane PLUS the optional human-name
        // handle plane absorbed from the retired `duckdns` module. Files owns
        // DuckFS bytes, loopback ports stay local. store-backed; the chain id
        // rides the store-seeded GENESIS CONFIG.
        gateway,
        // per-member notification queues; other modules deliver via follow-up
        // ops so a notification commits atomically with the causing event (P2).
        // store-backed over the host-constructed qmdb store.
        inbox,
        // the ROOT-CONTINUOUS files tenant: a wasm guest over the host-side
        // duckfs odb + refs backing (`files_wasm`). `("files", 1)` stays and the
        // cutover moves no root — pinned by `wasm_files_parity`.
        files: files_wasm(duckfs_dir.to_path_buf()).expect("duckfs open"),
        // the agent registry: a self-contained record book; its hook keeps
        // each agent's dispatch recipe in lockstep via the runs module.
        // store-backed over the host-constructed qmdb store; the saga
        // dead-letter + runs hook ids are compiled into the guest.
        agent,
        // the collaboration loop's actor: watches, engagement, composition,
        // dispatch, and response delivery — reads the registry by query.
        // adapter-ported; the whole production wiring (chat/saga/tagging/
        // dispatch/agent/tasks + the files/forge/pages builder chain) is
        // compiled into the guest.
        runs: genesis_runs_wasm(),
        // the first real wasm tenant: bytes-compatible with the retired native
        // implementation, so this cutover left the root-hash untouched.
        directory: genesis_directory_wasm(),
        // user-defined rules over chat posts: trusts the "chat" origin for hook
        // events and emits chat/tasks follow-ups. store-backed over the
        // host-constructed qmdb store; the chat/tasks/inbox lane ids are
        // compiled into the guest.
        automations,
    }
    .compose()
    .expect("genesis host")
}

/// the RESTORE twin of [`genesis_host`]: the disk substrates (qmdb modules,
/// forge's git repo) reopen themselves at their own committed positions; the
/// in-memory cohort installs its checkpoint snapshots, root-checked. the
/// recovery replay then rolls everything forward to the journal tip.
pub(super) async fn restore_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    manifest: &Manifest,
    blobs: blobstore::BlobHandle,
) -> Result<Host, String> {
    // store-backed wasm tenants restore like the other qmdb modules: the
    // stores reopen themselves at their committed positions and the wasm
    // wrapper computes root() straight from them (no snapshot install — see
    // [`pages_wasm`]).
    let pages = pages_wasm(Box::new(
        QmdbStore::init(context.child("pages"), "pages").await,
    ));
    let chat = chat_wasm(Box::new(
        QmdbStore::init(context.child("chat"), "chat").await,
    ));
    let agent = agent_wasm(Box::new(
        QmdbStore::init(context.child("agent"), "agent").await,
    ));
    let automations = automations_wasm(Box::new(
        QmdbStore::init(context.child("automations"), "automations").await,
    ));
    // governance and identity reopen like the other store-backed tenants;
    // each `__config` record is committed store state, so no re-seeding.
    let governance = governance_wasm(Box::new(
        QmdbStore::init(context.child("governance"), "governance").await,
    ));
    let identity = identity_wasm(Box::new(
        QmdbStore::init(context.child("identity"), "identity").await,
    ));
    let capability = capability_wasm(Box::new(
        QmdbStore::init(context.child("capability"), "capability").await,
    ));
    let gateway = gateway_wasm(Box::new(
        QmdbStore::init(context.child("gateway"), "gateway").await,
    ));
    let tagging = tagging_wasm(Box::new(
        QmdbStore::init(context.child("tagging"), "tagging").await,
    ));
    seed_genesis_components(&blobs);
    // forge shares the blob plane (see genesis_host) for Push materialization.
    let forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|e| format!("forge: {e}"))?
        .with_chat("chat");
    let snapshot_of = |id: &str| -> Result<(&[u8], StateRoot), String> {
        let bytes = manifest
            .snapshot(id)
            .ok_or_else(|| format!("checkpoint has no snapshot for module {id}"))?;
        let root = manifest
            .root(id)
            .ok_or_else(|| format!("checkpoint has no root for module {id}"))?;
        Ok((bytes, root))
    };

    // the membership registry reopens like the other store-backed tenants
    // (its store carries the genesis seed and every committed membership op).
    let valset = Valset::new(
        "valset",
        Box::new(QmdbStore::init(context.child("valset"), "valset").await),
    );
    // the submit-policy table reopens the same way (empty store = allow-all).
    let acl_table = acl_wasm(Box::new(
        QmdbStore::init(context.child("acl"), "acl").await,
    ));

    // the lifecycle module-code registry reopens like the other store-backed
    // tenants (its store carries the genesis seeds and every committed swap);
    // the wasm tenants are rebuilt on their EMBEDDED genesis components here,
    // and recovery's boot-time code reconciliation swaps them to the
    // committed active code.
    let lifecycle = Lifecycle::new(
        host::LIFECYCLE_MODULE_ID,
        Box::new(QmdbStore::init(context.child("lifecycle"), "lifecycle").await),
        "valset",
    );
    let inbox = inbox_wasm(Box::new(
        QmdbStore::init(context.child("inbox"), "inbox").await,
    ));
    let tasks = tasks_wasm(Box::new(
        QmdbStore::init(context.child("tasks"), "tasks").await,
    ));
    let dispatch = dispatch_wasm(Box::new(
        QmdbStore::init(context.child("dispatch"), "dispatch").await,
    ));

    let mut hello_wasm = genesis_hello_wasm();
    let (bytes, root) = snapshot_of(HELLO_WASM_MODULE_ID)?;
    hello_wasm
        .install(bytes, root)
        .map_err(|e| format!("{HELLO_WASM_MODULE_ID} install: {e}"))?;

    let saga = saga_wasm(Box::new(
        QmdbStore::init(context.child("saga"), "saga").await,
    ));

    // files is a duckfs-odb resolver module — NOT in the checkpoint's snapshot
    // set (like the qmdb modules above, which `init` from their own on-disk
    // stores). `files_wasm`'s `FilesOdbBacking::open` recovers committed refs,
    // durable height, and objects from the on-disk odb/refs envelope. Recovery
    // replays forward from that height, so reboot needs no checkpoint bytes or
    // object fetch here. The reopened root is `sha256(encode_refs)`.
    let files = files_wasm(duckfs_dir.to_path_buf()).map_err(|e| format!("duckfs open: {e}"))?;

    let mut runs = genesis_runs_wasm();
    let (bytes, root) = snapshot_of(RUNS_MODULE_ID)?;
    runs.install(bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    let mut directory = genesis_directory_wasm();
    let (bytes, root) = snapshot_of(DIRECTORY_MODULE_ID)?;
    directory
        .install(bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    let host = ProductionModules {
        pages,
        chat,
        forge,
        valset,
        acl: acl_table,
        governance,
        lifecycle,
        hello_wasm,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
        identity,
        gateway,
        inbox,
        files,
        agent,
        runs,
        directory,
        automations,
    }
    .compose()
    .map_err(|e| format!("restore host: {e}"))?;
    Ok(host)
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

/// rebuild EVERY production module from a peer's statesync service at
/// `manifest`'s boundary and compose them into a [`Host`], verified against
/// the manifest's root-hash. the disk substrates land under their canonical
/// ids in this process's storage root — this IS the node's state afterwards,
/// not a scratch copy. `attempt` disambiguates runtime child labels across
/// retries (a busy source moves its qmdb targets past the captured boundary;
/// the caller refetches the manifest and tries again, and metrics labels
/// must not collide).
pub(super) async fn sync_all_modules<C: statesync::SyncClient>(
    context: &commonware_runtime::tokio::Context,
    client: &C,
    manifest: &statesync::Manifest,
    substrates: SyncSubstrates<'_>,
    attempt: usize,
) -> Result<Host, String> {
    let SyncSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
    } = substrates;
    seed_genesis_components(&blobs);
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

    // store-backed wasm tenants join like the other qmdb modules: rebuild the
    // CONCRETE store at the manifest's pinned target (merkle-verified against
    // the committed root), then wrap the wasm module around it — the same
    // sync-then-inject lifecycle the native constructors had.
    let (target, resolver) = fetch_target("pages").await?;
    let pages = pages_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("pages")),
            "pages",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("chat").await?;
    let chat = chat_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("chat")),
            "chat",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("agent").await?;
    let agent = agent_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("agent")),
            "agent",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("automations").await?;
    let automations = automations_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("automations")),
            "automations",
            target,
            resolver,
        )
        .await?,
    ));

    // governance and identity join the same way; each `__config` record is
    // ordinary store state, so it arrives with the synced op range and the
    // rebuilt root commits to it exactly like the source's.
    let (target, resolver) = fetch_target("governance").await?;
    let governance = governance_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("governance")),
            "governance",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("identity").await?;
    let identity = identity_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("identity")),
            "identity",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("capability").await?;
    let capability = capability_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("capability")),
            "capability",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("gateway").await?;
    let gateway = gateway_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("gateway")),
            "gateway",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("tasks").await?;
    let tasks = tasks_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("tasks")),
            "tasks",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("tagging").await?;
    let tagging = tagging_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("tagging")),
            "tagging",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target(DISPATCH_MODULE_ID).await?;
    let dispatch = dispatch_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("dispatch")),
            "dispatch",
            target,
            resolver,
        )
        .await?,
    ));
    // snapshot lane: chunked bytes from the captured boundary, install gated
    // on the manifest root (verify-then-adopt inside each module).
    let snapshot_of = |module: &'static str| {
        let client = client.clone();
        let boundary = manifest.boundary_id();
        let root = entry_root(module);
        async move {
            let root = root?;
            let bytes = fetch_snapshot(&client, boundary, module)
                .await
                .map_err(|e| format!("{module} snapshot: {e}"))?;
            Ok::<_, String>((bytes, root))
        }
    };

    let (bytes, root) = snapshot_of(DIRECTORY_MODULE_ID).await?;
    let mut directory = genesis_directory_wasm();
    directory
        .install(&bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    // the membership registry joins like the other store-backed tenants:
    // rebuild the store at the manifest's pinned target (merkle-verified
    // against the committed root), then wrap the module around it.
    let (target, resolver) = fetch_target("valset").await?;
    let valset = Valset::new(
        "valset",
        Box::new(
            QmdbStore::sync_from(
                scratch_context.child(child_label("valset")),
                "valset",
                target,
                resolver,
            )
            .await?,
        ),
    );

    // the submit-policy table joins like the other store-backed tenants.
    let (target, resolver) = fetch_target("acl").await?;
    let acl_table = acl_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("acl")),
            "acl",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target("saga").await?;
    let saga = saga_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("saga")),
            "saga",
            target,
            resolver,
        )
        .await?,
    ));

    // the lifecycle module-code registry joins like the other store-backed
    // tenants: rebuild the store at the manifest's pinned target. the wasm
    // tenants join on their EMBEDDED genesis components — a post-swap
    // network's committed active hash differs, and the joiner's first code
    // reconciliation (before it applies any block) swaps them to the committed
    // components, fetched off the blob plane.
    let (target, resolver) = fetch_target("inbox").await?;
    let inbox = inbox_wasm(Box::new(
        QmdbStore::sync_from(
            scratch_context.child(child_label("inbox")),
            "inbox",
            target,
            resolver,
        )
        .await?,
    ));

    let (target, resolver) = fetch_target(host::LIFECYCLE_MODULE_ID).await?;
    let lifecycle = Lifecycle::new(
        host::LIFECYCLE_MODULE_ID,
        Box::new(
            QmdbStore::sync_from(
                scratch_context.child(child_label("lifecycle")),
                "lifecycle",
                target,
                resolver,
            )
            .await?,
        ),
        "valset",
    );

    let (bytes, root) = snapshot_of(HELLO_WASM_MODULE_ID).await?;
    let mut hello_wasm = genesis_hello_wasm();
    hello_wasm
        .install(&bytes, root)
        .map_err(|e| format!("{HELLO_WASM_MODULE_ID} install: {e}"))?;

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
    let files = files_wasm(files_scratch.dir().to_path_buf())
        .map_err(|e| format!("duckfs compose: {e}"))?;

    let (bytes, root) = snapshot_of(RUNS_MODULE_ID).await?;
    let mut runs = genesis_runs_wasm();
    runs.install(&bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|e| format!("forge init: {e}"))?
        .with_chat("chat");
    forge
        .install(&bytes, root)
        .map_err(|e| format!("forge install: {e}"))?;

    // compose and check THE property: the rebuilt root-hash IS the manifest's.
    // [`ProductionModules`] keeps this registry in lockstep with
    // [`genesis_host`] by construction — a missing module composes a
    // different root-hash and the join fails its final check.
    let mut host = ProductionModules {
        pages,
        chat,
        forge,
        valset,
        acl: acl_table,
        governance,
        lifecycle,
        hello_wasm,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
        identity,
        gateway,
        inbox,
        files,
        agent,
        runs,
        directory,
        automations,
    }
    .compose()
    .map_err(|e| format!("compose synced host: {e}"))?;
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
    host.register(Box::new(
        files_wasm(duckfs_dir.to_path_buf()).map_err(|e| format!("duckfs reopen: {e}"))?,
    ));
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
        "bb5657ad499d8422d0e5762bb4eddc65f3f01331f96d0d777e6586e201dd016c";

    /// The bindings [`GENESIS_ROOT_HASH`] is taken over. They are constants
    /// because they are NOT: each rides its module's store as a genesis
    /// `__config` record ([`seed_store_config`]), so a real network's invite
    /// namespace and chain id put it on its own root by design. Pinning a hash
    /// only says anything against fixed ones.
    const PIN_BINDINGS: NetworkBindings<'static> = NetworkBindings {
        invite: b"parity-test",
        identity_chain_id: "parity-test",
    };

    /// Compose the production genesis host in a throwaway storage root and
    /// return `(module ids sorted, root hash hex)` — everything both pins below
    /// need, so neither has to keep its own copy of the construction.
    ///
    /// Production runs this root future on macOS's ~8 MiB process stack. Run
    /// the test twin on the same budget: libtest's 2 MiB worker stack is just
    /// below this full 20-module composition's debug-build requirement.
    const GENESIS_TEST_STACK_BYTES: usize = 8 * 1024 * 1024;

    fn genesis_facts() -> (Vec<String>, String) {
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

    fn compose_genesis_facts() -> (Vec<String>, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        let forge_repo = dir.path().join("forge");
        let duckfs_dir = dir.path().join("duckfs");
        let cfg = commonware_runtime::tokio::Config::default()
            .with_storage_directory(dir.path().join("storage"));
        let executor = commonware_runtime::tokio::Runner::new(cfg);
        executor.start(|context| async move {
            let host = genesis_host(
                &context,
                &forge_repo,
                &duckfs_dir,
                &[],
                PIN_BINDINGS,
                blobstore::BlobHandle::default(),
            )
            .await;
            // module_roots iterates the host's BTreeMap — sorted by id.
            let ids = host.module_roots().into_iter().map(|(id, _)| id).collect();
            (ids, hex(&host.root_hash()))
        })
    }

    /// the registry ↔ topology parity pin. [`ProductionModules`] already forces
    /// genesis, restore, and state sync onto one module set at compile time;
    /// this test pins that set to `MODULE_IDS` — the `production` selection of
    /// the single-source `topology` the status/index surfaces iterate — so
    /// adding a module to one but not the other fails here instead of silently
    /// misreporting.
    #[test]
    fn genesis_registry_matches_module_ids() {
        let (got, _root) = genesis_facts();
        let mut want: Vec<String> = MODULE_IDS.iter().map(|s| s.to_string()).collect();
        want.sort_unstable();
        assert_eq!(got, want);
    }

    /// THE consensus pin: the production genesis root hash is a constant.
    ///
    /// It is the only ABSOLUTE one in the tree, and until it existed every claim
    /// that "the root hash did not move" was relative and therefore weak.
    /// `bin/simnode/tests/topology_set.rs` pins the 14-module NATIVE sim
    /// composition — which excludes `capability`, `hello`, `governance` and
    /// `lifecycle`, and is not what a node runs. And `git diff crates/modules/`
    /// on a committed tree is EMPTY BY CONSTRUCTION, so quoting it proves
    /// nothing at all. Neither would have noticed a module's bytes changing.
    ///
    /// ## the mechanism, because it surprises everyone once
    ///
    /// What this covers is wider than the module SET. [`seeded_lifecycle`]
    /// commits `sha256(component.wasm)` for every wasm tenant into the lifecycle
    /// module's MerkleStore, so each guest's CODE DIGEST is consensus state
    /// itself. That means a module's SOURCE is consensus-relevant the moment its
    /// component is rebuilt — even for a change that alters no behaviour, even a
    /// comment — and it means `make wasm-modules` can ship a sixteen-module flag
    /// day as a side effect of touching one guest. That is correct, and it is
    /// exactly the event that must never happen silently.
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
        let (_ids, root) = genesis_facts();
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
