//! Application-host construction, restoration, and state synchronization.
//!
//! This module owns the module registry and the durable swap from scratch
//! state into the canonical host. The live node loop only consumes the three
//! lifecycle operations and output adapter exported below.

use clients::Clients;
use commonware_cryptography::ed25519;
use commonware_runtime::Supervisor as _;
use dispatch::DispatchModule;
use duckfs_disk::SyncScratch;
use files::Files;
use forge::Forge;
use host::Host;
use kv::Kv;
use modreg::Modreg;
use recovery::Manifest;
use sdk::StateRoot;
use sha2::Digest as _;
use statesync::{
    fetch_snapshot,
    qmdb::{QmdbStore, RemoteQmdbResolver},
};
use upgrade::Upgrade;
use valset::Valset;
use wasm_host::WasmModule;

use crate::constants::{
    CLIENTS_MODULE_ACTIVATION_VERSION, current_state_schema_fingerprint,
    native_v1_state_schema_fingerprint, pre_clients_state_schema_fingerprint,
};
use crate::util::hex;

mod native_v1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StateSchemaRoute {
    Exact,
    AddClientsAtV1,
    NativeV1,
}

fn classify_recovery_state_schema(
    found: Option<[u8; 32]>,
    current_version: u32,
) -> Result<StateSchemaRoute, String> {
    if found == Some(native_v1_state_schema_fingerprint()) && current_version == 1 {
        return Ok(StateSchemaRoute::NativeV1);
    }
    classify_state_schema(found, current_version)
}

fn classify_state_schema(
    found: Option<[u8; 32]>,
    current_version: u32,
) -> Result<StateSchemaRoute, String> {
    if found == Some(current_state_schema_fingerprint()) {
        return Ok(StateSchemaRoute::Exact);
    }
    if found == Some(pre_clients_state_schema_fingerprint())
        && current_version < CLIENTS_MODULE_ACTIVATION_VERSION
    {
        return Ok(StateSchemaRoute::AddClientsAtV1);
    }
    recovery::check_state_schema(found, current_state_schema_fingerprint())
        .map(|()| StateSchemaRoute::Exact)
        .map_err(|error| error.to_string())
}

pub(super) fn preflight_recovery_schema(manifest: &Manifest) -> Result<StateSchemaRoute, String> {
    let route = classify_recovery_state_schema(manifest.state_schema, manifest.current_version)?;
    if route == StateSchemaRoute::NativeV1 && manifest.pending_upgrade.is_some() {
        return Err("native v1 recovery requires pending_upgrade: none".into());
    }
    Ok(route)
}

pub(super) fn preflight_sync_schema(
    manifest: &statesync::Manifest,
) -> Result<StateSchemaRoute, String> {
    classify_recovery_state_schema(Some(manifest.state_schema), manifest.current_version)
}

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
    include_bytes!("../../../crates/examples/hello-wasm/component.wasm");

/// the genesis-constant id the reference wasm module registers under.
const HELLO_WASM_MODULE_ID: &str = "hello";

/// the blobstore-backed [`host::CodeSource`]: component bytes for a code swap
/// are content-addressed chunks on the node's blob plane (staged there before
/// the governance schedule, exactly like a forge Push packfile). a hash the
/// store lacks is a `None` — the boundary fails closed rather than forking.
pub(super) struct BlobCodeSource(pub(super) blobstore::BlobHandle);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for BlobCodeSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        self.0.get_chunk(&digest)
    }
}

/// the directory module's GENESIS component — the first REAL production tenant
/// of the module runtime. bytes-compatible with the retired native
/// implementation (`directory-wasm` stores the same keys/values under the same
/// canonical encoding), so root(), snapshots, and pre-cutover workspace
/// restores are all continuous across the cutover.
const DIRECTORY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/directory-wasm/component.wasm");

/// the genesis-constant id the directory module registers under.
const DIRECTORY_MODULE_ID: &str = "directory";

/// the vaults module's GENESIS component — the first ADAPTER-ported tenant:
/// the guest compiles the NATIVE `vaults` crate to wasm and persists its
/// canonical snapshot as one host-KV value per dispatch, so the module's
/// logic is single-sourced while the state schema breaks (revision 2 in
/// `MODULE_STATE_SCHEMAS`; beta networks re-genesis, no back-compat shim).
const VAULTS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/vaults-wasm/component.wasm");

/// the genesis-constant id the vaults module registers under.
const VAULTS_MODULE_ID: &str = "vaults";

/// jobs / inbox / tasks GENESIS components — adapter-ported tenants like
/// vaults (native crate compiled into the guest; canonical snapshot persisted
/// through the host-KV store; revision 2 in `MODULE_STATE_SCHEMAS`).
const JOBS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/jobs-wasm/component.wasm");
const JOBS_MODULE_ID: &str = "jobs";
const INBOX_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/inbox-wasm/component.wasm");
const INBOX_MODULE_ID: &str = "inbox";
const TASKS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/tasks-wasm/component.wasm");
const TASKS_MODULE_ID: &str = "tasks";

/// tagging / capability / duckdns — adapter-ported tenants whose ops resolve
/// SIBLING READS (valset standing, identity bindings, content-module roots)
/// through the runtime's memoized replay. upgrade deliberately stays NATIVE:
/// its Advance decides over frozen end-of-block committed state, a surface the
/// wit world's staged-over-committed reads cannot represent (kernel
/// coordinators — valset, modreg, upgrade — gate the machinery itself).
const TAGGING_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/tagging-wasm/component.wasm");
const TAGGING_MODULE_ID: &str = "tagging";
const CAPABILITY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/capability-wasm/component.wasm");
const CAPABILITY_MODULE_ID: &str = "capability";
const DUCKDNS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/duckdns-wasm/component.wasm");
const DUCKDNS_MODULE_ID: &str = "duckdns";

/// identity / gateway / governance — adapter-ported tenants whose native
/// constructors take PER-NETWORK parameters (the identity chain id, the invite
/// binding). a wasm component is fixed bytes, so those parameters travel as
/// GENESIS CONFIG: the builders below install an initial store carrying the
/// reserved `__config` key (`sdk::genesis_config`) at construction, and the
/// guest decodes it per dispatch. the config is consensus state — identical on
/// every node, in the module root (hence the app-hash) from genesis, and
/// carried by checkpoint snapshots like any other store key.
const IDENTITY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/identity-wasm/component.wasm");
const IDENTITY_MODULE_ID: &str = "identity";
const GATEWAY_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/gateway-wasm/component.wasm");
const GATEWAY_MODULE_ID: &str = "gateway";
const GOVERNANCE_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/governance-wasm/component.wasm");
const GOVERNANCE_MODULE_ID: &str = "governance";

/// saga / agent / automations — adapter-ported tenants of the async engine's
/// deterministic half and its consumers. every decision in their execute
/// paths reads staged-over-committed state, so the whole-state fold is
/// behavior-identical (pinned by their parity proofs); their work-order
/// events, P6 callbacks, registry hooks, and chat-hook probe reads all cross
/// the wit seam unchanged. DISPATCH deliberately stays NATIVE: its read
/// facade (`DispatchQuery::Dispatch`, `PendingDeliveries`) serves
/// COMMITTED-ONLY state by design — runs' mid-block `lease_holder` read
/// depends on it — and the adapter's staged-fold cannot represent a
/// committed-only view (the same class of frozen-committed read that keeps
/// upgrade native).
const SAGA_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/saga-wasm/component.wasm");
const SAGA_MODULE_ID: &str = "saga";
const AGENT_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/agent-wasm/component.wasm");
const AGENT_MODULE_ID: &str = "agent";
const AUTOMATIONS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/automations-wasm/component.wasm");
const AUTOMATIONS_MODULE_ID: &str = "automations";

/// runs — the FINAL adapter-ported tenant: the collaboration loop's actor.
/// every decision in its handle paths reads staged-over-committed (the
/// watch/pending-entry/session accessors shadow the committed maps with the
/// block's overlays), so the whole-state fold is behavior-identical (pinned
/// by `wasm_runs_parity`). its ten collaborator ids — chat, saga, tagging,
/// dispatch, agent, tasks, jobs, plus the files/forge/pages builder chain —
/// are genesis-constant and compiled into the guest (the exact production
/// constructor these builders used to call natively). the dispatch reads it
/// depends on (`turn_taken`'s permanent record, the session lane's
/// `lease_holder`) are COMMITTED-ONLY by dispatch's design — dispatch stays
/// native and answers committed-only regardless of caller, so the guest's
/// host-routed queries read exactly what the native module read. the
/// delivered-runs ring (`RecentRuns`) — derived per-node state outside the
/// NATIVE root/snapshot — persists through the guest's own `__history` key
/// (the app's runs client and the dogfood receipt lane read it), so it rides
/// the wasm root and snapshots like everything else the guest keeps.
const RUNS_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/runs-wasm/component.wasm");
const RUNS_MODULE_ID: &str = "runs";

/// pages / chat — the first STORE-BACKED wasm tenants: the native crates are
/// pure logic over an injected `sdk::MerkleStore`, so the guests compile that
/// SAME logic over the adapter's `WitStore` while the REAL qmdb store stays
/// host-side (`WasmModule::with_store`). unlike the whole-state adapter
/// ports there is no snapshot re-encoding: the module root IS the store's
/// merkle root, byte-identical across the cutover (state schema revision
/// STAYS 1 — see `constants::MODULE_STATE_SCHEMAS`), and pre-cutover
/// workspaces reopen unchanged. NOTE the `.with_tagging("tagging")` wiring
/// moved INTO the guests — the host builder chain drops it.
const PAGES_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/pages-wasm/component.wasm");
const PAGES_MODULE_ID: &str = "pages";
const CHAT_WASM_COMPONENT: &[u8] =
    include_bytes!("../../../crates/examples/chat-wasm/component.wasm");
const CHAT_MODULE_ID: &str = "chat";

/// genesis-seed the code registry: every wasm tenant's initial active code
/// hash, identical on every node (the embedded components ARE the hashes'
/// preimages). shared by the genesis / restore / state-sync host builders so
/// all three compose the same registry shape.
fn seeded_modreg() -> Modreg {
    let mut modreg = Modreg::new(host::MODREG_MODULE_ID, "valset");
    modreg.seed(
        HELLO_WASM_MODULE_ID,
        sha2::Sha256::digest(HELLO_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        DIRECTORY_MODULE_ID,
        sha2::Sha256::digest(DIRECTORY_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        VAULTS_MODULE_ID,
        sha2::Sha256::digest(VAULTS_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        JOBS_MODULE_ID,
        sha2::Sha256::digest(JOBS_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        INBOX_MODULE_ID,
        sha2::Sha256::digest(INBOX_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        TASKS_MODULE_ID,
        sha2::Sha256::digest(TASKS_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        TAGGING_MODULE_ID,
        sha2::Sha256::digest(TAGGING_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        CAPABILITY_MODULE_ID,
        sha2::Sha256::digest(CAPABILITY_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        DUCKDNS_MODULE_ID,
        sha2::Sha256::digest(DUCKDNS_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        IDENTITY_MODULE_ID,
        sha2::Sha256::digest(IDENTITY_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        GATEWAY_MODULE_ID,
        sha2::Sha256::digest(GATEWAY_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        GOVERNANCE_MODULE_ID,
        sha2::Sha256::digest(GOVERNANCE_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        PAGES_MODULE_ID,
        sha2::Sha256::digest(PAGES_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        CHAT_MODULE_ID,
        sha2::Sha256::digest(CHAT_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        SAGA_MODULE_ID,
        sha2::Sha256::digest(SAGA_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        AGENT_MODULE_ID,
        sha2::Sha256::digest(AGENT_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        AUTOMATIONS_MODULE_ID,
        sha2::Sha256::digest(AUTOMATIONS_WASM_COMPONENT).to_vec(),
    );
    modreg.seed(
        RUNS_MODULE_ID,
        sha2::Sha256::digest(RUNS_WASM_COMPONENT).to_vec(),
    );
    modreg
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
    blobs.put_chunk(VAULTS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(JOBS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(INBOX_WASM_COMPONENT.to_vec());
    blobs.put_chunk(TASKS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(TAGGING_WASM_COMPONENT.to_vec());
    blobs.put_chunk(CAPABILITY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(DUCKDNS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(IDENTITY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(GATEWAY_WASM_COMPONENT.to_vec());
    blobs.put_chunk(GOVERNANCE_WASM_COMPONENT.to_vec());
    blobs.put_chunk(PAGES_WASM_COMPONENT.to_vec());
    blobs.put_chunk(CHAT_WASM_COMPONENT.to_vec());
    blobs.put_chunk(SAGA_WASM_COMPONENT.to_vec());
    blobs.put_chunk(AGENT_WASM_COMPONENT.to_vec());
    blobs.put_chunk(AUTOMATIONS_WASM_COMPONENT.to_vec());
    blobs.put_chunk(RUNS_WASM_COMPONENT.to_vec());
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

/// the vaults module at its GENESIS code (same reconciliation story as
/// [`genesis_hello_wasm`]). revision 2: the adapter port persists the native
/// canonical snapshot under the host-KV encoding — a state-schema break from
/// the native module's root, fenced by the recovery/state-sync preflight.
fn genesis_vaults_wasm() -> WasmModule {
    WasmModule::from_bytes(VAULTS_MODULE_ID, VAULTS_WASM_COMPONENT)
        .expect("embedded vaults component loads")
        .with_state_schema_revision(2)
}

/// jobs / inbox / tasks at their GENESIS code (adapter-ported, revision 2 —
/// see [`genesis_vaults_wasm`]).
fn genesis_jobs_wasm() -> WasmModule {
    WasmModule::from_bytes(JOBS_MODULE_ID, JOBS_WASM_COMPONENT)
        .expect("embedded jobs component loads")
        .with_state_schema_revision(2)
}

fn genesis_inbox_wasm() -> WasmModule {
    WasmModule::from_bytes(INBOX_MODULE_ID, INBOX_WASM_COMPONENT)
        .expect("embedded inbox component loads")
        .with_state_schema_revision(2)
}

fn genesis_tasks_wasm() -> WasmModule {
    WasmModule::from_bytes(TASKS_MODULE_ID, TASKS_WASM_COMPONENT)
        .expect("embedded tasks component loads")
        .with_state_schema_revision(2)
}

/// tagging / capability / duckdns at their GENESIS code (adapter-ported,
/// revision 2 — see [`genesis_vaults_wasm`]).
fn genesis_tagging_wasm() -> WasmModule {
    WasmModule::from_bytes(TAGGING_MODULE_ID, TAGGING_WASM_COMPONENT)
        .expect("embedded tagging component loads")
        .with_state_schema_revision(2)
}

fn genesis_capability_wasm() -> WasmModule {
    WasmModule::from_bytes(CAPABILITY_MODULE_ID, CAPABILITY_WASM_COMPONENT)
        .expect("embedded capability component loads")
        .with_state_schema_revision(2)
}

fn genesis_duckdns_wasm() -> WasmModule {
    WasmModule::from_bytes(DUCKDNS_MODULE_ID, DUCKDNS_WASM_COMPONENT)
        .expect("embedded duckdns component loads")
        .with_state_schema_revision(2)
}

/// saga / agent / automations at their GENESIS code (adapter-ported; saga and
/// automations are revision 2, while agent's later role tail makes it revision
/// 3). the sibling wiring — saga's valset/capability assignment reads, agent's
/// saga dead-letter + runs hook, automations' chat/tasks/inbox lanes — is
/// genesis-constant and compiled into the guests (the exact production
/// constructors these builders used to call natively).
fn genesis_saga_wasm() -> WasmModule {
    WasmModule::from_bytes(SAGA_MODULE_ID, SAGA_WASM_COMPONENT)
        .expect("embedded saga component loads")
        .with_state_schema_revision(2)
}

fn genesis_agent_wasm() -> WasmModule {
    WasmModule::from_bytes(AGENT_MODULE_ID, AGENT_WASM_COMPONENT)
        .expect("embedded agent component loads")
        .with_state_schema_revision(3)
}

fn genesis_automations_wasm() -> WasmModule {
    WasmModule::from_bytes(AUTOMATIONS_MODULE_ID, AUTOMATIONS_WASM_COMPONENT)
        .expect("embedded automations component loads")
        .with_state_schema_revision(2)
}

/// runs at its GENESIS code (adapter-ported — see the component const's doc).
/// revision 3: the native canonical snapshot (itself at revision 2 since the
/// session section landed) is persisted as one host-KV value, so the encoding
/// breaks shape again at cutover.
fn genesis_runs_wasm() -> WasmModule {
    WasmModule::from_bytes(RUNS_MODULE_ID, RUNS_WASM_COMPONENT)
        .expect("embedded runs component loads")
        .with_state_schema_revision(3)
}

/// pages at its GENESIS code over the host-constructed store (a fresh/reopened
/// `QmdbStore::init`, or a `QmdbStore::sync_from` at a verified root — all
/// three lifecycles hand the store in the same way, exactly as they handed it
/// to the native constructor). REVISION STAYS 1: the committed encoding is the
/// store's own op log and the module root is its merkle root, byte-identical
/// across the cutover (pinned by `wasm_pages_parity`), so no schema fence and
/// no re-genesis.
fn pages_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(PAGES_MODULE_ID, PAGES_WASM_COMPONENT, store)
        .expect("embedded pages component loads")
}

/// chat at its GENESIS code over the host-constructed store (store-backed,
/// revision stays 1 — see [`pages_wasm`]; pinned by `wasm_chat_parity`).
fn chat_wasm(store: Box<dyn sdk::MerkleStore>) -> WasmModule {
    WasmModule::with_store(CHAT_MODULE_ID, CHAT_WASM_COMPONENT, store)
        .expect("embedded chat component loads")
}

/// a wasm tenant at its GENESIS code with a host-installed GENESIS-CONFIG
/// store: the component loads, then adopts an initial snapshot whose one
/// entry is the reserved `__config` key carrying the canonically-encoded
/// per-network parameters. install is verify-then-adopt against the
/// host-computed root, so the constructed module's root commits to the config
/// from block zero (genesis roots honestly differ per network). the RESTORE /
/// STATE-SYNC paths construct through the same builder and then install the
/// checkpoint snapshot over it — the config rides in those bytes, so the
/// interim config-only store is simply replaced by the same-config checkpoint.
fn genesis_config_wasm(
    module_id: &str,
    component: &[u8],
    params: &[(&str, &[u8])],
    what: &str,
) -> WasmModule {
    let mut module = WasmModule::from_bytes(module_id, component)
        .unwrap_or_else(|e| panic!("embedded {what} component loads: {e}"))
        .with_state_schema_revision(2);
    let config = sdk::genesis_config::encode_config(params);
    let (bytes, root) = wasm_host::initial_state(&[(sdk::genesis_config::CONFIG_KEY, &config)]);
    module
        .install(&bytes, root)
        .unwrap_or_else(|e| panic!("{what} genesis config installs: {e}"));
    module
}

/// identity at its GENESIS code + config (adapter-ported, revision 2 — see
/// [`genesis_vaults_wasm`]): the per-network chain id rides `__config`.
fn genesis_identity_wasm(bindings: NetworkBindings<'_>) -> WasmModule {
    genesis_config_wasm(
        IDENTITY_MODULE_ID,
        IDENTITY_WASM_COMPONENT,
        &[("chain_id", bindings.identity_chain_id.as_bytes())],
        "identity",
    )
}

/// gateway at its GENESIS code + config: routes are chain-scoped, so the same
/// per-network chain id rides `__config`.
fn genesis_gateway_wasm(bindings: NetworkBindings<'_>) -> WasmModule {
    genesis_config_wasm(
        GATEWAY_MODULE_ID,
        GATEWAY_WASM_COMPONENT,
        &[("chain_id", bindings.identity_chain_id.as_bytes())],
        "gateway",
    )
}

/// governance at its GENESIS code + config: the invite binding every token
/// and join proof verify against rides `__config` (the sibling wiring —
/// valset/upgrade/identity/modreg — is genesis-constant and compiled into the
/// guest like every other port's sibling ids).
fn genesis_governance_wasm(bindings: NetworkBindings<'_>) -> WasmModule {
    genesis_config_wasm(
        GOVERNANCE_MODULE_ID,
        GOVERNANCE_WASM_COMPONENT,
        &[("invite", bindings.invite)],
        "governance",
    )
}

pub(super) fn run_output_sink(registry: noded::RunOutputRegistry) -> capability_host::OutputSink {
    std::sync::Arc::new(move |ctx, line| {
        let Some(run_key) = ctx.run_key.as_deref() else {
            return;
        };
        let stream = match line.stream {
            capability_host::OutputStream::Stdout => noded::RunStream::Stdout,
            capability_host::OutputStream::Stderr => noded::RunStream::Stderr,
        };
        registry.append(run_key, stream, line.line);
    })
}

/// the production module registry: ONE named field per module, so genesis,
/// restore, and state sync compose the SAME set by construction — adding a
/// module means adding a field here, and every lifecycle fails to compile
/// until it builds one. `constants::MODULE_IDS` mirrors this set for the
/// status surfaces; the parity test below pins the two together.
struct ProductionModules {
    kv: Kv,
    pages: WasmModule,
    chat: WasmModule,
    forge: Forge,
    valset: Valset,
    clients: Clients,
    governance: WasmModule,
    upgrade: Upgrade,
    modreg: Modreg,
    hello_wasm: WasmModule,
    saga: WasmModule,
    capability: WasmModule,
    dispatch: DispatchModule,
    tagging: WasmModule,
    tasks: WasmModule,
    vaults: WasmModule,
    identity: WasmModule,
    duckdns: WasmModule,
    gateway: WasmModule,
    inbox: WasmModule,
    files: Files,
    jobs: WasmModule,
    agent: WasmModule,
    runs: WasmModule,
    directory: WasmModule,
    automations: WasmModule,
}

impl ProductionModules {
    /// compose the registry into a [`Host`]. registration order is NOT
    /// consensus-relevant (the host keys modules in a `BTreeMap`) — only the
    /// module set and each module's constructed state compose the app-hash.
    fn compose(self) -> Result<Host, sdk::Error> {
        Host::genesis(vec![
            Box::new(self.kv),
            Box::new(self.pages),
            Box::new(self.chat),
            Box::new(self.forge),
            Box::new(self.valset),
            Box::new(self.clients),
            Box::new(self.governance),
            Box::new(self.upgrade),
            Box::new(self.modreg),
            Box::new(self.hello_wasm),
            Box::new(self.saga),
            Box::new(self.capability),
            Box::new(self.dispatch),
            Box::new(self.tagging),
            Box::new(self.tasks),
            Box::new(self.vaults),
            Box::new(self.identity),
            Box::new(self.duckdns),
            Box::new(self.gateway),
            Box::new(self.inbox),
            Box::new(self.files),
            Box::new(self.jobs),
            Box::new(self.agent),
            Box::new(self.runs),
            Box::new(self.directory),
            Box::new(self.automations),
        ])
    }
}

/// the PRODUCTION module set — genesis state, identical on every node (a
/// different set composes a different app-hash and the network forks at
/// genesis). system infrastructure (kv, valset seeded with the genesis
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
    let kv = Kv::new(
        "kv",
        Box::new(QmdbStore::init(context.child("kv"), "kv").await),
    );
    // pages/chat are STORE-BACKED wasm tenants: the host still constructs the
    // concrete qmdb stores exactly as before — only the executor wrapped
    // around them changed (and `.with_tagging` moved into the guests).
    let pages = pages_wasm(Box::new(
        QmdbStore::init(context.child("pages"), "pages").await,
    ));
    let chat = chat_wasm(Box::new(
        QmdbStore::init(context.child("chat"), "chat").await,
    ));
    seed_genesis_components(&blobs);
    // forge shares the blob plane so a Push's packfile (staged on the blob
    // lane before submit) can materialize locally; the pack never touches root.
    let forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .expect("forge init")
        .with_chat("chat");
    let mut valset = Valset::new("valset");
    // genesis-seed the validator set from config — deterministic and identical
    // on every node, so membership is IN consensus state from block zero (the
    // substrate epoch cutover + governance will drive).
    for v in genesis_validators {
        valset.insert(v.as_ref().to_vec());
    }
    // the client ACL: empty at genesis. a redeemed role=Client invite records a
    // key here (governance emits ClientsMsg::Grant). SEPARATE from valset by
    // design — a client never gets statesync/quorum standing.
    let clients = Clients::new("clients");
    ProductionModules {
        kv,
        pages,
        chat,
        forge,
        valset,
        clients,
        // governance is the SOLE authorized author of valset AND client-ACL
        // changes: member proposals + ballots, deterministic tally, follow-up
        // membership ops, and the redeem-time client grant. adapter-ported;
        // the invite binding rides its GENESIS CONFIG, the clients sibling id
        // is compiled into the guest like the rest.
        governance: genesis_governance_wasm(bindings),
        // the no-downtime upgrade coordinator: holds the at-most-one pending
        // upgrade + per-validator readiness set (valset-gated). its mere
        // presence in the registry is its genesis app-hash contribution.
        upgrade: Upgrade::new("upgrade", "valset"),
        // the module code registry: the consensus commitment to WHICH component
        // each hot-swappable wasm module runs, seeded with the genesis hashes.
        // governance schedules height-gated swaps into it; the host realizes
        // them at the boundary through the blobstore-backed CodeSource.
        modreg: seeded_modreg(),
        // the reference wasm module — the live-update machinery's first tenant.
        hello_wasm: genesis_hello_wasm(),
        // capability-aware strict leases: a saga whose trigger names a
        // capability is assigned over that tag's announced providers, and
        // only the assignee's result lands. an UNASSIGNED attempt (empty
        // provider pool) accepts no result at all: its WorkerRequest is an
        // announcement a capable node must first claim via `SagaMsg::Accept`.
        // adapter-ported; the valset/capability wiring and the Strict policy
        // are compiled into the guest.
        saga: genesis_saga_wasm(),
        // the network-wide registry of node host capabilities ("codex",
        // "claude", ...): member-gated self-announcements, so every node holds
        // an identical view of who can run what. its genesis contribution is an
        // empty registry (ZERO root) until nodes announce.
        capability: genesis_capability_wasm(),
        // the task plane: recipe manifests + capability-routed dispatch with
        // next-block result delivery (the host's DeliverPending injection).
        dispatch: DispatchModule::new("dispatch", "saga"),
        // the engagement plane: content modules report tags, subscriber
        // modules receive engagement events — router only, module-agnostic.
        tagging: genesis_tagging_wasm(),
        tasks: genesis_tasks_wasm(),
        // the first ADAPTER-ported wasm tenant: the native vaults logic
        // compiled to wasm, whole-state persisted per dispatch (see
        // `genesis_vaults_wasm`).
        vaults: genesis_vaults_wasm(),
        // the deterministic user->nodes binding registry: certificates are
        // chain-scoped (this network's chain id, riding its GENESIS CONFIG),
        // member-gated binds via valset, and account display names have this
        // single canonical owner.
        identity: genesis_identity_wasm(bindings),
        duckdns: genesis_duckdns_wasm(),
        // Identity-signed, monotonic gateway routes. DuckDNS owns optional
        // human names, Files owns DuckFS bytes, and loopback ports stay local.
        // adapter-ported; the chain id rides its GENESIS CONFIG.
        gateway: genesis_gateway_wasm(bindings),
        // per-member notification queues; other modules deliver via follow-up
        // ops so a notification commits atomically with the causing event (P2).
        inbox: genesis_inbox_wasm(),
        files: Files::open("files", duckfs_dir.to_path_buf()).expect("duckfs open"),
        jobs: genesis_jobs_wasm(),
        // the agent registry: a self-contained record book; its hook keeps
        // each agent's dispatch recipe in lockstep via the runs module.
        // adapter-ported; the saga dead-letter + runs hook ids are compiled
        // into the guest.
        agent: genesis_agent_wasm(),
        // the collaboration loop's actor: watches, engagement, composition,
        // dispatch, and response delivery — reads the registry by query.
        // adapter-ported; the whole production wiring (chat/saga/tagging/
        // dispatch/agent/tasks/jobs + the files/forge/pages builder chain) is
        // compiled into the guest.
        runs: genesis_runs_wasm(),
        // the first real wasm tenant: bytes-compatible with the retired native
        // implementation, so this cutover left the app-hash untouched.
        directory: genesis_directory_wasm(),
        // user-defined rules over chat posts: trusts the "chat" origin for hook
        // events and emits chat/tasks follow-ups. adapter-ported; the
        // chat/tasks/inbox lane ids are compiled into the guest.
        automations: genesis_automations_wasm(),
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
    bindings: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
) -> Result<Host, String> {
    // Must precede every module/storage constructor below. A clean-break
    // mismatch is classification, not a failed install attempt, and the
    // archived workspace must remain byte-for-byte untouched.
    let schema_route = preflight_recovery_schema(manifest)?;
    if schema_route == StateSchemaRoute::NativeV1 {
        return native_v1::restore_host(context, forge_repo, duckfs_dir, manifest, bindings, blobs)
            .await;
    }
    let kv = Kv::new(
        "kv",
        Box::new(QmdbStore::init(context.child("kv"), "kv").await),
    );
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

    let mut valset = Valset::new("valset");
    let (bytes, root) = snapshot_of("valset")?;
    valset
        .install(bytes, root)
        .map_err(|e| format!("valset install: {e}"))?;

    let mut clients = Clients::new("clients");
    if schema_route == StateSchemaRoute::Exact {
        let (bytes, root) = snapshot_of("clients")?;
        clients
            .install(bytes, root)
            .map_err(|e| format!("clients install: {e}"))?;
    }

    // wasm tenants with GENESIS CONFIG restore like any other wasm module:
    // construct through the genesis builder (config-only initial store), then
    // adopt the checkpoint snapshot — the config rides in those bytes, so the
    // install simply replaces the interim store with the same-config one.
    // NOTE on the schema route: the guest compiles `.with_clients` (gate open
    // from v0), matching the Exact route. the AddClientsAtV1 route cannot
    // reach this tree — the wasm cutover's revision fences refuse pre-clients
    // workspaces before any route is derived (beta re-genesis, no shim).
    let mut governance = genesis_governance_wasm(bindings);
    let (bytes, root) = snapshot_of(GOVERNANCE_MODULE_ID)?;
    governance
        .install(bytes, root)
        .map_err(|e| format!("governance install: {e}"))?;

    let mut upgrade = Upgrade::new("upgrade", "valset");
    let (bytes, root) = snapshot_of("upgrade")?;
    upgrade
        .install(bytes, root)
        .map_err(|e| format!("upgrade install: {e}"))?;

    // the code registry + the wasm module restore like any in-memory module:
    // construct at genesis shape, adopt the checkpoint snapshot. the wasm module
    // is rebuilt on its EMBEDDED genesis component here — recovery's boot-time
    // code reconciliation swaps it to the checkpoint's committed active code
    // (state installs independently of code, so the roots check out either way).
    let mut modreg = Modreg::new(host::MODREG_MODULE_ID, "valset");
    let (bytes, root) = snapshot_of(host::MODREG_MODULE_ID)?;
    modreg
        .install(bytes, root)
        .map_err(|e| format!("modreg install: {e}"))?;

    let mut hello_wasm = genesis_hello_wasm();
    let (bytes, root) = snapshot_of(HELLO_WASM_MODULE_ID)?;
    hello_wasm
        .install(bytes, root)
        .map_err(|e| format!("{HELLO_WASM_MODULE_ID} install: {e}"))?;

    let mut saga = genesis_saga_wasm();
    let (bytes, root) = snapshot_of(SAGA_MODULE_ID)?;
    saga.install(bytes, root)
        .map_err(|e| format!("saga install: {e}"))?;

    let mut capability = genesis_capability_wasm();
    let (bytes, root) = snapshot_of(CAPABILITY_MODULE_ID)?;
    capability
        .install(bytes, root)
        .map_err(|e| format!("capability install: {e}"))?;

    let mut dispatch = DispatchModule::new("dispatch", "saga");
    let (bytes, root) = snapshot_of("dispatch")?;
    dispatch
        .install(bytes, root)
        .map_err(|e| format!("dispatch install: {e}"))?;

    let mut tagging = genesis_tagging_wasm();
    let (bytes, root) = snapshot_of(TAGGING_MODULE_ID)?;
    tagging
        .install(bytes, root)
        .map_err(|e| format!("tagging install: {e}"))?;

    let mut tasks = genesis_tasks_wasm();
    let (bytes, root) = snapshot_of(TASKS_MODULE_ID)?;
    tasks
        .install(bytes, root)
        .map_err(|e| format!("tasks install: {e}"))?;

    // the checkpoint bytes are the WasmModule host-KV snapshot (the adapter
    // port's own canonical encoding), so a wasm-era checkpoint reinstalls
    // self-consistently; pre-cutover checkpoints are fenced off by the
    // state-schema preflight above (revision 2).
    let mut vaults = genesis_vaults_wasm();
    let (bytes, root) = snapshot_of(VAULTS_MODULE_ID)?;
    vaults
        .install(bytes, root)
        .map_err(|e| format!("vaults install: {e}"))?;

    let mut identity = genesis_identity_wasm(bindings);
    let (bytes, root) = snapshot_of(IDENTITY_MODULE_ID)?;
    identity
        .install(bytes, root)
        .map_err(|e| format!("identity install: {e}"))?;

    let mut duckdns = genesis_duckdns_wasm();
    let (bytes, root) = snapshot_of(DUCKDNS_MODULE_ID)?;
    duckdns
        .install(bytes, root)
        .map_err(|e| format!("duckdns install: {e}"))?;

    let mut gateway = genesis_gateway_wasm(bindings);
    let (bytes, root) = snapshot_of(GATEWAY_MODULE_ID)?;
    gateway
        .install(bytes, root)
        .map_err(|e| format!("gateway install: {e}"))?;

    let mut inbox = genesis_inbox_wasm();
    let (bytes, root) = snapshot_of(INBOX_MODULE_ID)?;
    inbox
        .install(bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    // files is a duckfs-odb resolver module — NOT in the checkpoint's snapshot
    // set (like the qmdb modules above, which `init` from their own on-disk
    // stores). `Files::open` already recovers its committed refs, durable height,
    // and objects from the on-disk odb/refs envelope, and recovery replays
    // forward from that height — so a reboot needs no checkpoint bytes and no
    // object fetch here.
    let files =
        Files::open("files", duckfs_dir.to_path_buf()).map_err(|e| format!("duckfs open: {e}"))?;

    let mut jobs = genesis_jobs_wasm();
    let (bytes, root) = snapshot_of(JOBS_MODULE_ID)?;
    jobs.install(bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let mut agent = genesis_agent_wasm();
    let (bytes, root) = snapshot_of(AGENT_MODULE_ID)?;
    agent
        .install(bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let mut runs = genesis_runs_wasm();
    let (bytes, root) = snapshot_of(RUNS_MODULE_ID)?;
    runs.install(bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    // pre-cutover checkpoints install unchanged: the wasm directory's snapshot
    // encoding is byte-identical to the retired native implementation's.
    let mut directory = genesis_directory_wasm();
    let (bytes, root) = snapshot_of(DIRECTORY_MODULE_ID)?;
    directory
        .install(bytes, root)
        .map_err(|e| format!("directory install: {e}"))?;

    let mut automations = genesis_automations_wasm();
    let (bytes, root) = snapshot_of(AUTOMATIONS_MODULE_ID)?;
    automations
        .install(bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    let mut host = ProductionModules {
        kv,
        pages,
        chat,
        forge,
        valset,
        clients,
        governance,
        upgrade,
        modreg,
        hello_wasm,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
        vaults,
        identity,
        duckdns,
        gateway,
        inbox,
        files,
        jobs,
        agent,
        runs,
        directory,
        automations,
    }
    .compose()
    .map_err(|e| format!("restore host: {e}"))?;
    if schema_route == StateSchemaRoute::AddClientsAtV1 {
        host.defer_module_until("clients", CLIENTS_MODULE_ACTIVATION_VERSION)
            .map_err(|e| format!("restore host: {e}"))?;
    }
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
/// only by the verified promotion after `sync_all_modules`' composite app-hash
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
/// the manifest's app-hash. the disk substrates land under their canonical
/// ids in this process's storage root — this IS the node's state afterwards,
/// not a scratch copy. `attempt` disambiguates runtime child labels across
/// retries (a busy source moves its qmdb targets past the captured boundary;
/// the caller refetches the manifest and tries again, and metrics labels
/// must not collide).
pub(super) async fn sync_all_modules<C: statesync::SyncClient>(
    context: &commonware_runtime::tokio::Context,
    client: &C,
    manifest: &statesync::Manifest,
    bindings: NetworkBindings<'_>,
    substrates: SyncSubstrates<'_>,
    attempt: usize,
) -> Result<Host, String> {
    // Refuse before creating scratch/canonical stores. A peer can only offer
    // state for the exact binary schema this joiner understands.
    let schema_route = preflight_sync_schema(manifest)?;
    if schema_route == StateSchemaRoute::NativeV1 {
        return native_v1::sync_all_modules(
            context, client, manifest, bindings, substrates, attempt,
        )
        .await;
    }
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

    let (target, resolver) = fetch_target("kv").await?;
    let kv = Kv::new(
        "kv",
        Box::new(
            QmdbStore::sync_from(
                scratch_context.child(child_label("kv")),
                "kv",
                target,
                resolver,
            )
            .await?,
        ),
    );

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

    let (bytes, root) = snapshot_of("valset").await?;
    let mut valset = Valset::new("valset");
    valset
        .install(&bytes, root)
        .map_err(|e| format!("valset install: {e}"))?;

    let mut clients = Clients::new("clients");
    if schema_route == StateSchemaRoute::Exact {
        let (bytes, root) = snapshot_of("clients").await?;
        clients
            .install(&bytes, root)
            .map_err(|e| format!("clients install: {e}"))?;
    }

    let (bytes, root) = snapshot_of(SAGA_MODULE_ID).await?;
    let mut saga = genesis_saga_wasm();
    saga.install(&bytes, root)
        .map_err(|e| format!("saga install: {e}"))?;

    let (bytes, root) = snapshot_of(CAPABILITY_MODULE_ID).await?;
    let mut capability = genesis_capability_wasm();
    capability
        .install(&bytes, root)
        .map_err(|e| format!("capability install: {e}"))?;

    let (bytes, root) = snapshot_of("dispatch").await?;
    let mut dispatch = DispatchModule::new("dispatch", "saga");
    dispatch
        .install(&bytes, root)
        .map_err(|e| format!("dispatch install: {e}"))?;

    let (bytes, root) = snapshot_of(TAGGING_MODULE_ID).await?;
    let mut tagging = genesis_tagging_wasm();
    tagging
        .install(&bytes, root)
        .map_err(|e| format!("tagging install: {e}"))?;

    // wasm tenants with GENESIS CONFIG join like any other wasm module:
    // construct through the genesis builder (config-only initial store), then
    // adopt the served snapshot, root-checked — the config rides in it (the
    // schema-route note on the restore path above applies here too).
    let (bytes, root) = snapshot_of(GOVERNANCE_MODULE_ID).await?;
    let mut governance = genesis_governance_wasm(bindings);
    governance
        .install(&bytes, root)
        .map_err(|e| format!("governance install: {e}"))?;

    let (bytes, root) = snapshot_of("upgrade").await?;
    let mut upgrade = Upgrade::new("upgrade", "valset");
    upgrade
        .install(&bytes, root)
        .map_err(|e| format!("upgrade install: {e}"))?;

    // the code registry + the wasm module join like any in-memory module: adopt
    // the served snapshot, root-checked. the wasm module joins on its EMBEDDED
    // genesis component — a post-swap network's committed active hash differs,
    // and the joiner's first code reconciliation (before it applies any block)
    // swaps it to the committed component, fetched off the blob plane.
    let (bytes, root) = snapshot_of(host::MODREG_MODULE_ID).await?;
    let mut modreg = Modreg::new(host::MODREG_MODULE_ID, "valset");
    modreg
        .install(&bytes, root)
        .map_err(|e| format!("modreg install: {e}"))?;

    let (bytes, root) = snapshot_of(HELLO_WASM_MODULE_ID).await?;
    let mut hello_wasm = genesis_hello_wasm();
    hello_wasm
        .install(&bytes, root)
        .map_err(|e| format!("{HELLO_WASM_MODULE_ID} install: {e}"))?;

    let (bytes, root) = snapshot_of(TASKS_MODULE_ID).await?;
    let mut tasks = genesis_tasks_wasm();
    tasks
        .install(&bytes, root)
        .map_err(|e| format!("tasks install: {e}"))?;

    let (bytes, root) = snapshot_of(VAULTS_MODULE_ID).await?;
    let mut vaults = genesis_vaults_wasm();
    vaults
        .install(&bytes, root)
        .map_err(|e| format!("vaults install: {e}"))?;

    let (bytes, root) = snapshot_of(IDENTITY_MODULE_ID).await?;
    let mut identity = genesis_identity_wasm(bindings);
    identity
        .install(&bytes, root)
        .map_err(|e| format!("identity install: {e}"))?;

    let (bytes, root) = snapshot_of(DUCKDNS_MODULE_ID).await?;
    let mut duckdns = genesis_duckdns_wasm();
    duckdns
        .install(&bytes, root)
        .map_err(|e| format!("duckdns install: {e}"))?;

    let (bytes, root) = snapshot_of(GATEWAY_MODULE_ID).await?;
    let mut gateway = genesis_gateway_wasm(bindings);
    gateway
        .install(&bytes, root)
        .map_err(|e| format!("gateway install: {e}"))?;

    let (bytes, root) = snapshot_of(INBOX_MODULE_ID).await?;
    let mut inbox = genesis_inbox_wasm();
    inbox
        .install(&bytes, root)
        .map_err(|e| format!("inbox install: {e}"))?;

    // files is a duckfs-odb resolver module: its refs image AND its
    // content-addressed objects both ride the Module/`serve_sync` lane. a fresh
    // joiner's odb is EMPTY, so install the boundary refs (root-verified) at the
    // sync-target height and then loop GetObjects to full object possession —
    // the snapshot lane would leave this node refs-only (every file listed, not
    // one byte readable). the sync lands in an ATTEMPT-scoped scratch dir
    // (`duckfs_scratch_a{attempt}`, mirroring the qmdb scratch namespaces);
    // the canonical `duckfs_dir` is written only by the verified promotion
    // after the composite app-hash gate below (#219).
    let files_scratch =
        SyncScratch::prepare(duckfs_dir, attempt).map_err(|e| format!("duckfs scratch: {e}"))?;
    let mut files = Files::open("files", files_scratch.dir().to_path_buf())
        .map_err(|e| format!("duckfs open: {e}"))?;
    let files_root = entry_root("files")?;
    let files_lane = statesync::ClientModuleLane::new(client.clone(), manifest.boundary_id());
    statesync::sync_object_possession(
        &files_lane,
        "files",
        files_root,
        manifest.height,
        &mut FilesOdb(&mut files),
        duckfs_core::MAX_SYNC_IDS,
    )
    .await
    .map_err(|e| format!("files sync: {e}"))?;

    let (bytes, root) = snapshot_of(JOBS_MODULE_ID).await?;
    let mut jobs = genesis_jobs_wasm();
    jobs.install(&bytes, root)
        .map_err(|e| format!("jobs install: {e}"))?;

    let (bytes, root) = snapshot_of(AGENT_MODULE_ID).await?;
    let mut agent = genesis_agent_wasm();
    agent
        .install(&bytes, root)
        .map_err(|e| format!("agent install: {e}"))?;

    let (bytes, root) = snapshot_of(RUNS_MODULE_ID).await?;
    let mut runs = genesis_runs_wasm();
    runs.install(&bytes, root)
        .map_err(|e| format!("runs install: {e}"))?;

    let (bytes, root) = snapshot_of(AUTOMATIONS_MODULE_ID).await?;
    let mut automations = genesis_automations_wasm();
    automations
        .install(&bytes, root)
        .map_err(|e| format!("automations install: {e}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|e| format!("forge init: {e}"))?
        .with_chat("chat");
    // set the dual-path branch selector to the SERVED boundary version BEFORE
    forge
        .install(&bytes, root)
        .map_err(|e| format!("forge install: {e}"))?;

    // compose and check THE property: the rebuilt app-hash IS the manifest's.
    // [`ProductionModules`] keeps this registry in lockstep with
    // [`genesis_host`] by construction — a missing module composes a
    // different app-hash and the join fails its final check.
    let mut host = ProductionModules {
        kv,
        pages,
        chat,
        forge,
        valset,
        clients,
        governance,
        upgrade,
        modreg,
        hello_wasm,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
        vaults,
        identity,
        duckdns,
        gateway,
        inbox,
        files,
        jobs,
        agent,
        runs,
        directory,
        automations,
    }
    .compose()
    .map_err(|e| format!("compose synced host: {e}"))?;
    if schema_route == StateSchemaRoute::AddClientsAtV1 {
        host.defer_module_until("clients", CLIENTS_MODULE_ACTIVATION_VERSION)
            .map_err(|e| format!("compose synced host: {e}"))?;
    }
    // realize the served boundary version into EVERY dual-path module's branch
    // selector so `root()` (and with it the app-hash check below) recomputes over
    // the boundary's format — the state-sync analogue of the activation hook the
    // live/recovery paths run. NON-hashed; idempotent for forge (set pre-install
    // above); baseline no-op before Phase 9.
    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
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
        Files::open("files", duckfs_dir.to_path_buf())
            .map_err(|e| format!("duckfs reopen: {e}"))?,
    ));
    // re-realize the boundary version over the swapped registry (idempotent),
    // then re-check THE property against the canonical-backed composition.
    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "canonical duckfs reopen composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
        ));
    }
    Ok(host)
}

#[cfg(test)]
mod tests {
    use commonware_runtime::Runner as _;

    use super::*;
    use crate::constants::{
        MODULE_IDS, MODULE_STATE_SCHEMAS, native_v1_state_schema_fingerprint,
        pre_clients_state_schema_fingerprint,
    };

    #[test]
    fn pre_clients_schema_has_one_explicit_height_gated_route() {
        // the ONE migratable fingerprint: the current schema minus `clients`
        // (re-pinned at the wasm cutover — the pre-cutover 77418fd6… world is
        // fail-closed like every other historical schema).
        assert_eq!(
            crate::config::hex_bytes(&pre_clients_state_schema_fingerprint()),
            "2f9bc45b9abc691c65d9f6175f7a3aba39962dc8b6fce27ba207162887a60375"
        );
        assert_eq!(
            classify_state_schema(Some(pre_clients_state_schema_fingerprint()), 0),
            Ok(StateSchemaRoute::AddClientsAtV1)
        );
        assert!(
            classify_state_schema(Some(pre_clients_state_schema_fingerprint()), 1).is_err(),
            "a legacy schema already claiming v1 must fail closed"
        );
        assert_eq!(
            classify_state_schema(Some(current_state_schema_fingerprint()), 1),
            Ok(StateSchemaRoute::Exact)
        );
    }

    #[test]
    fn native_v1_schema_has_one_exact_compatibility_route() {
        assert_eq!(
            crate::config::hex_bytes(&native_v1_state_schema_fingerprint()),
            "363608db2d6271c5fba5b2dd55b1de6043aeeb101fb3e0db43d60550c7aca1af"
        );
        assert_eq!(
            classify_recovery_state_schema(Some(native_v1_state_schema_fingerprint()), 1),
            Ok(StateSchemaRoute::NativeV1)
        );
        assert!(
            classify_recovery_state_schema(Some(native_v1_state_schema_fingerprint()), 0).is_err()
        );
        assert!(
            classify_recovery_state_schema(Some(native_v1_state_schema_fingerprint()), 2).is_err()
        );
        let mut unknown = native_v1_state_schema_fingerprint();
        unknown[0] ^= 1;
        assert!(classify_recovery_state_schema(Some(unknown), 1).is_err());
    }

    /// the registry ↔ `MODULE_IDS` parity pin. [`ProductionModules`] already
    /// forces genesis, restore, and state sync onto one module set at compile
    /// time; this test pins that set to the `constants::MODULE_IDS` copy the
    /// status/index surfaces iterate, so adding a module to one but not the
    /// other fails here instead of silently misreporting.
    #[test]
    fn genesis_registry_matches_module_ids() {
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
                NetworkBindings {
                    invite: b"parity-test",
                    identity_chain_id: "parity-test",
                },
                blobstore::BlobHandle::default(),
            )
            .await;
            // module_roots iterates the host's BTreeMap — sorted by id.
            let got: Vec<String> = host.module_roots().into_iter().map(|(id, _)| id).collect();
            let mut want: Vec<String> = MODULE_IDS.iter().map(|s| s.to_string()).collect();
            want.sort_unstable();
            assert_eq!(got, want);
            let declared: Vec<(String, u32)> = MODULE_STATE_SCHEMAS
                .iter()
                .map(|(id, revision)| ((*id).to_string(), *revision))
                .collect();
            assert_eq!(host.state_schema(), declared);
            assert_eq!(
                host.state_schema_fingerprint(),
                current_state_schema_fingerprint()
            );
        });
    }

    #[test]
    fn pre_clients_checkpoint_restores_without_rewriting_legacy_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let forge_repo = dir.path().join("forge");
        let duckfs_dir = dir.path().join("duckfs");
        let cfg = commonware_runtime::tokio::Config::default()
            .with_storage_directory(dir.path().join("storage"));
        let executor = commonware_runtime::tokio::Runner::new(cfg);
        executor.start(|context| async move {
            let mut legacy = genesis_host(
                &context,
                &forge_repo,
                &duckfs_dir,
                &[],
                NetworkBindings {
                    invite: b"legacy-clients-route",
                    identity_chain_id: "legacy-clients-route",
                },
                blobstore::BlobHandle::default(),
            )
            .await;
            legacy
                .defer_module_until("clients", CLIENTS_MODULE_ACTIVATION_VERSION)
                .expect("construct legacy registry");
            let legacy_hash = legacy.app_hash();
            let manifest = Manifest::capture(
                &legacy,
                None,
                0,
                0,
                Vec::new(),
                Vec::new(),
                None,
                0,
                None,
                0,
                1,
            )
            .expect("capture legacy checkpoint");
            assert_eq!(
                manifest.state_schema,
                Some(pre_clients_state_schema_fingerprint())
            );
            drop(legacy);

            let mut restored = restore_host(
                &context,
                &forge_repo,
                &duckfs_dir,
                &manifest,
                NetworkBindings {
                    invite: b"legacy-clients-route",
                    identity_chain_id: "legacy-clients-route",
                },
                blobstore::BlobHandle::default(),
            )
            .await
            .expect("restore legacy checkpoint through clients route");
            restored.set_active_version(0);
            assert_eq!(restored.app_hash(), legacy_hash);
            assert!(restored.module_root("clients").is_none());

            restored.set_active_version(CLIENTS_MODULE_ACTIVATION_VERSION);
            assert!(restored.module_root("clients").is_some());
            assert_ne!(
                restored.app_hash(),
                legacy_hash,
                "the clients root joins the app hash only at the v1 boundary"
            );
        });
    }

    #[test]
    fn incompatible_schema_refuses_before_opening_or_installing_workspace_state() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source_forge = dir.path().join("source-forge");
        let source_duckfs = dir.path().join("source-duckfs");
        let workspace = dir.path().join("preserved-workspace");
        std::fs::create_dir(&workspace).expect("workspace");
        let sentinel = workspace.join("identity.key");
        std::fs::write(&sentinel, b"preserve-me").expect("sentinel");
        let target_forge = workspace.join("forge-repo");
        let target_duckfs = workspace.join("duckfs");
        let cfg = commonware_runtime::tokio::Config::default()
            .with_storage_directory(dir.path().join("runtime"));
        let executor = commonware_runtime::tokio::Runner::new(cfg);
        executor.start(|context| async move {
            let host = genesis_host(
                &context,
                &source_forge,
                &source_duckfs,
                &[],
                NetworkBindings {
                    invite: b"schema-preflight-test",
                    identity_chain_id: "schema-preflight-test",
                },
                blobstore::BlobHandle::default(),
            )
            .await;
            let mut manifest = Manifest::capture(
                &host,
                None,
                0,
                0,
                Vec::new(),
                Vec::new(),
                None,
                0,
                None,
                0,
                1,
            )
            .expect("capture");
            manifest.state_schema = Some([0xFF; 32]);

            let error = match restore_host(
                &context,
                &target_forge,
                &target_duckfs,
                &manifest,
                NetworkBindings {
                    invite: b"schema-preflight-test",
                    identity_chain_id: "schema-preflight-test",
                },
                blobstore::BlobHandle::default(),
            )
            .await
            {
                Ok(_) => panic!("mismatch must precede module install"),
                Err(error) => error,
            };
            assert!(error.contains(recovery::STATE_SCHEMA_INCOMPATIBLE_MARKER));
            assert_eq!(
                std::fs::read(&sentinel).expect("sentinel survives"),
                b"preserve-me"
            );
            assert!(!target_forge.exists(), "Forge substrate was never opened");
            assert!(!target_duckfs.exists(), "DuckFS substrate was never opened");
        });
    }
}
