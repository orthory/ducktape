//! Adapter for the exact native production registry that runs the protocol-v1
//! dogfood network. It has no genesis entry point and accepts only the pinned
//! native-v1 schema: this is a binary roll, not a state migration.

use agent::AgentModule;
use automations::Automations;
use capability::CapabilityRegistry;
use chat::Chat;
use commonware_runtime::Supervisor as _;
use directory::Directory;
use dispatch::DispatchModule;
use duckdns::DuckDns;
use duckfs_disk::SyncScratch;
use files::Files;
use forge::Forge;
// the FROZEN routes-only gateway module: on the v1 network the `.duck` handle
// plane rode a SEPARATE `duckdns` module, so this registry reconstructs the
// routes-only `gateway` snapshot AND the separate `duckdns` snapshot. The live
// path uses the MERGED `gateway::Gateway`.
use gateway::LegacyGateway;
use governance::Governance;
use host::Host;
use identity::Identity;
use inbox::Inbox;
use jobs::Jobs;
use kv::Kv;
use modreg::Modreg;
use pages::Pages;
use recovery::Manifest;
use runs::RunsModule;
use saga::{LeasePolicy, SagaModule};
use sdk::StateRoot;
use statesync::{
    fetch_snapshot,
    qmdb::{QmdbStore, RemoteQmdbResolver},
};
use tagging::TaggingModule;
use tasks::Tasks;
use upgrade::Upgrade;
use valset::Valset;
use wasm_host::WasmModule;

use super::{
    FilesOdb, NetworkBindings, SyncSubstrates, genesis_hello_wasm, seed_genesis_components,
};
use crate::util::hex;

struct NativeV1Modules {
    kv: Kv,
    pages: Pages,
    chat: Chat,
    forge: Forge,
    valset: Valset,
    clients: clients::Clients,
    governance: Governance,
    upgrade: Upgrade,
    modreg: Modreg,
    hello: WasmModule,
    saga: SagaModule,
    capability: CapabilityRegistry,
    dispatch: DispatchModule,
    tagging: TaggingModule,
    tasks: Tasks,
    identity: Identity,
    duckdns: DuckDns,
    gateway: LegacyGateway,
    inbox: Inbox,
    files: Files,
    jobs: Jobs,
    agent: AgentModule,
    runs: RunsModule,
    directory: Directory,
    automations: Automations,
}

impl NativeV1Modules {
    fn compose(self) -> Result<Host, sdk::Error> {
        let mut host = Host::genesis(vec![
            Box::new(self.kv),
            Box::new(self.pages),
            Box::new(self.chat),
            Box::new(self.forge),
            Box::new(self.valset),
            Box::new(self.clients),
            Box::new(self.governance),
            Box::new(self.upgrade),
            Box::new(self.modreg),
            Box::new(self.hello),
            Box::new(self.saga),
            Box::new(self.capability),
            Box::new(self.dispatch),
            Box::new(self.tagging),
            Box::new(self.tasks),
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
        ])?;
        // the compatibility composition still admits post-genesis modules once
        // the protocol version that carries them is active.
        host.set_module_factory(Box::new(super::WasmModuleFactory));
        Ok(host)
    }
}

fn ensure_upgrade_matches(
    current_version: u32,
    pending: Option<(&str, u64, u32)>,
    upgrade: &Upgrade,
) -> Result<(), String> {
    let (snapshot_version, snapshot_pending) = upgrade.committed_coordinates();
    if snapshot_version != current_version {
        return Err(format!(
            "upgrade snapshot protocol_v{snapshot_version} disagrees with manifest protocol_v{current_version}"
        ));
    }
    let snapshot_pending = snapshot_pending.as_ref().map(|upgrade| {
        (
            upgrade.name.as_str(),
            upgrade.activation_height,
            upgrade.to_version,
        )
    });
    if snapshot_pending != pending {
        return Err("upgrade snapshot disagrees with manifest pending_upgrade".into());
    }
    Ok(())
}

fn recovery_control_modules(manifest: &Manifest) -> Result<(Upgrade, Modreg), String> {
    if manifest.pending_upgrade.is_some() {
        return Err("native v1 recovery requires pending_upgrade: none".into());
    }
    let snapshot_of = |id: &str| -> Result<(&[u8], StateRoot), String> {
        Ok((
            manifest
                .snapshot(id)
                .ok_or_else(|| format!("checkpoint has no snapshot for module {id}"))?,
            manifest
                .root(id)
                .ok_or_else(|| format!("checkpoint has no root for module {id}"))?,
        ))
    };

    let mut upgrade = Upgrade::new("upgrade", "valset");
    let (bytes, root) = snapshot_of("upgrade")?;
    upgrade
        .install(bytes, root)
        .map_err(|error| format!("upgrade install: {error}"))?;
    ensure_upgrade_matches(manifest.current_version, None, &upgrade)?;

    let mut modreg = Modreg::new(host::MODREG_MODULE_ID, "valset").with_legacy_v1_state();
    let (bytes, root) = snapshot_of(host::MODREG_MODULE_ID)?;
    modreg
        .install(bytes, root)
        .map_err(|error| format!("modreg install: {error}"))?;
    if modreg.has_pending_swaps() {
        return Err("native v1 recovery requires no pending module code swaps".into());
    }
    Ok((upgrade, modreg))
}

pub(super) async fn restore_host(
    context: &commonware_runtime::tokio::Context,
    forge_repo: &std::path::Path,
    duckfs_dir: &std::path::Path,
    manifest: &Manifest,
    bindings: NetworkBindings<'_>,
    blobs: blobstore::BlobHandle,
) -> Result<Host, String> {
    // Validate mutable boundary coordinators before opening QMDB, Forge,
    // DuckFS, or the blob plane. No new-schema transition is admitted here.
    let (upgrade, modreg) = recovery_control_modules(manifest)?;
    let kv = Kv::new(
        "kv",
        Box::new(QmdbStore::init(context.child("kv"), "kv").await),
    );
    let pages = Pages::new(
        "pages",
        Box::new(QmdbStore::init(context.child("pages"), "pages").await),
    )
    .with_tagging("tagging");
    let chat = Chat::new(
        "chat",
        Box::new(QmdbStore::init(context.child("chat"), "chat").await),
    )
    .with_tagging("tagging");

    seed_genesis_components(&blobs);
    let forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|error| format!("forge: {error}"))?
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
        .map_err(|error| format!("valset install: {error}"))?;

    let mut clients = clients::Clients::new("clients");
    let (bytes, root) = snapshot_of("clients")?;
    clients
        .install(bytes, root)
        .map_err(|error| format!("clients install: {error}"))?;

    let mut governance = Governance::new("governance", "valset", "upgrade", "identity")
        .with_invite_binding(bindings.invite)
        .with_clients("clients")
        .with_modreg(host::MODREG_MODULE_ID);
    let (bytes, root) = snapshot_of("governance")?;
    governance
        .install(bytes, root)
        .map_err(|error| format!("governance install: {error}"))?;

    let mut hello = genesis_hello_wasm();
    let (bytes, root) = snapshot_of("hello")?;
    hello
        .install(bytes, root)
        .map_err(|error| format!("hello install: {error}"))?;

    let mut saga = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
    let (bytes, root) = snapshot_of("saga")?;
    saga.install(bytes, root)
        .map_err(|error| format!("saga install: {error}"))?;

    let mut capability =
        CapabilityRegistry::new("capability", Some("valset".into())).with_legacy_v1_state();
    let (bytes, root) = snapshot_of("capability")?;
    capability
        .install(bytes, root)
        .map_err(|error| format!("capability install: {error}"))?;

    let mut dispatch = DispatchModule::new("dispatch", "saga");
    let (bytes, root) = snapshot_of("dispatch")?;
    dispatch
        .install(bytes, root)
        .map_err(|error| format!("dispatch install: {error}"))?;

    let mut tagging = TaggingModule::new("tagging").with_direct_owner("runs");
    let (bytes, root) = snapshot_of("tagging")?;
    tagging
        .install(bytes, root)
        .map_err(|error| format!("tagging install: {error}"))?;

    let mut tasks = Tasks::new("tasks");
    let (bytes, root) = snapshot_of("tasks")?;
    tasks
        .install(bytes, root)
        .map_err(|error| format!("tasks install: {error}"))?;

    let mut identity = Identity::new(
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id.to_string(),
    );
    let (bytes, root) = snapshot_of("identity")?;
    identity
        .install(bytes, root)
        .map_err(|error| format!("identity install: {error}"))?;

    let mut duckdns = DuckDns::new("duckdns", "identity", Some("valset".into()));
    let (bytes, root) = snapshot_of("duckdns")?;
    duckdns
        .install(bytes, root)
        .map_err(|error| format!("duckdns install: {error}"))?;

    let mut gateway = LegacyGateway::new(
        "gateway",
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id,
    );
    let (bytes, root) = snapshot_of("gateway")?;
    gateway
        .install(bytes, root)
        .map_err(|error| format!("gateway install: {error}"))?;

    let mut inbox = Inbox::new("inbox");
    let (bytes, root) = snapshot_of("inbox")?;
    inbox
        .install(bytes, root)
        .map_err(|error| format!("inbox install: {error}"))?;

    let files = Files::open("files", duckfs_dir.to_path_buf())
        .map_err(|error| format!("files: {error}"))?;

    let mut jobs = Jobs::new("jobs");
    let (bytes, root) = snapshot_of("jobs")?;
    jobs.install(bytes, root)
        .map_err(|error| format!("jobs install: {error}"))?;

    let mut agent = AgentModule::new("agent", "saga", Some("runs".into()))
        .with_legacy_v1_state();
    let (bytes, root) = snapshot_of("agent")?;
    agent
        .install(bytes, root)
        .map_err(|error| format!("agent install: {error}"))?;

    let mut runs = RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
    .with_files_module("files")
    .with_sink_forge("forge")
    .with_pages_module("pages");
    let (bytes, root) = snapshot_of("runs")?;
    runs.install(bytes, root)
        .map_err(|error| format!("runs install: {error}"))?;

    let mut directory = Directory::new("directory");
    let (bytes, root) = snapshot_of("directory")?;
    directory
        .install(bytes, root)
        .map_err(|error| format!("directory install: {error}"))?;

    let mut automations = Automations::new("automations", "chat", "tasks", "inbox");
    let (bytes, root) = snapshot_of("automations")?;
    automations
        .install(bytes, root)
        .map_err(|error| format!("automations install: {error}"))?;

    NativeV1Modules {
        kv,
        pages,
        chat,
        forge,
        valset,
        clients,
        governance,
        upgrade,
        modreg,
        hello,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
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
    .map_err(|error| format!("restore native v1 host: {error}"))
}

/// Rebuild the exact protocol-v1 native registry from a peer. The schema and
/// protocol-version gate runs in the parent before this function; the two
/// mutable-boundary gates below run before any disk scratch is created.
pub(super) async fn sync_all_modules<C: statesync::SyncClient>(
    context: &commonware_runtime::tokio::Context,
    client: &C,
    manifest: &statesync::Manifest,
    bindings: NetworkBindings<'_>,
    substrates: SyncSubstrates<'_>,
    attempt: usize,
) -> Result<Host, String> {
    if manifest.pending_upgrade.is_some() {
        return Err("native v1 state sync requires pending_upgrade: none".into());
    }

    let entry_root = |module: &str| -> Result<StateRoot, String> {
        Ok(manifest
            .entry(module)
            .ok_or_else(|| format!("module {module} missing from the manifest"))?
            .root)
    };
    let snapshot_of = |module: &'static str| {
        let client = client.clone();
        let boundary = manifest.boundary_id();
        let root = entry_root(module);
        async move {
            let root = root?;
            let bytes = fetch_snapshot(&client, boundary, module)
                .await
                .map_err(|error| format!("{module} snapshot: {error}"))?;
            Ok::<_, String>((bytes, root))
        }
    };

    let (bytes, root) = snapshot_of("upgrade").await?;
    let mut upgrade = Upgrade::new("upgrade", "valset");
    upgrade
        .install(&bytes, root)
        .map_err(|error| format!("upgrade install: {error}"))?;
    ensure_upgrade_matches(manifest.current_version, None, &upgrade)?;

    // A native-v1 code swap uses the old height-only pending shape. Refuse it
    // before creating qmdb/DuckFS scratch: mixed old/new code-plane semantics
    // are not part of this exact binary-roll route.
    let (bytes, root) = snapshot_of(host::MODREG_MODULE_ID).await?;
    let mut modreg = Modreg::new(host::MODREG_MODULE_ID, "valset").with_legacy_v1_state();
    modreg
        .install(&bytes, root)
        .map_err(|error| format!("modreg install: {error}"))?;
    if modreg.has_pending_swaps() {
        return Err("native v1 state sync requires no pending module code swaps".into());
    }

    let SyncSubstrates {
        forge_repo,
        duckfs_dir,
        blobs,
    } = substrates;
    seed_genesis_components(&blobs);

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
        pinned
            .to_sync_target()
            .map_err(|error| format!("{module} {error}"))
    };
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

    let (target, resolver) = fetch_target("pages").await?;
    let pages = Pages::new(
        "pages",
        Box::new(
            QmdbStore::sync_from(
                scratch_context.child(child_label("pages")),
                "pages",
                target,
                resolver,
            )
            .await?,
        ),
    )
    .with_tagging("tagging");

    let (target, resolver) = fetch_target("chat").await?;
    let chat = Chat::new(
        "chat",
        Box::new(
            QmdbStore::sync_from(
                scratch_context.child(child_label("chat")),
                "chat",
                target,
                resolver,
            )
            .await?,
        ),
    )
    .with_tagging("tagging");

    let (bytes, root) = snapshot_of("directory").await?;
    let mut directory = Directory::new("directory");
    directory
        .install(&bytes, root)
        .map_err(|error| format!("directory install: {error}"))?;

    let (bytes, root) = snapshot_of("valset").await?;
    let mut valset = Valset::new("valset");
    valset
        .install(&bytes, root)
        .map_err(|error| format!("valset install: {error}"))?;

    let (bytes, root) = snapshot_of("clients").await?;
    let mut clients = clients::Clients::new("clients");
    clients
        .install(&bytes, root)
        .map_err(|error| format!("clients install: {error}"))?;

    let (bytes, root) = snapshot_of("saga").await?;
    let mut saga = SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict);
    saga.install(&bytes, root)
        .map_err(|error| format!("saga install: {error}"))?;

    let (bytes, root) = snapshot_of("capability").await?;
    let mut capability =
        CapabilityRegistry::new("capability", Some("valset".into())).with_legacy_v1_state();
    capability
        .install(&bytes, root)
        .map_err(|error| format!("capability install: {error}"))?;

    let (bytes, root) = snapshot_of("dispatch").await?;
    let mut dispatch = DispatchModule::new("dispatch", "saga");
    dispatch
        .install(&bytes, root)
        .map_err(|error| format!("dispatch install: {error}"))?;

    let (bytes, root) = snapshot_of("tagging").await?;
    let mut tagging = TaggingModule::new("tagging").with_direct_owner("runs");
    tagging
        .install(&bytes, root)
        .map_err(|error| format!("tagging install: {error}"))?;

    let (bytes, root) = snapshot_of("governance").await?;
    let mut governance = Governance::new("governance", "valset", "upgrade", "identity")
        .with_invite_binding(bindings.invite)
        .with_clients("clients")
        .with_modreg(host::MODREG_MODULE_ID);
    governance
        .install(&bytes, root)
        .map_err(|error| format!("governance install: {error}"))?;

    let (bytes, root) = snapshot_of("hello").await?;
    let mut hello = genesis_hello_wasm();
    hello
        .install(&bytes, root)
        .map_err(|error| format!("hello install: {error}"))?;

    let (bytes, root) = snapshot_of("tasks").await?;
    let mut tasks = Tasks::new("tasks");
    tasks
        .install(&bytes, root)
        .map_err(|error| format!("tasks install: {error}"))?;

    let (bytes, root) = snapshot_of("identity").await?;
    let mut identity = Identity::new(
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id.to_string(),
    );
    identity
        .install(&bytes, root)
        .map_err(|error| format!("identity install: {error}"))?;

    let (bytes, root) = snapshot_of("duckdns").await?;
    let mut duckdns = DuckDns::new("duckdns", "identity", Some("valset".into()));
    duckdns
        .install(&bytes, root)
        .map_err(|error| format!("duckdns install: {error}"))?;

    let (bytes, root) = snapshot_of("gateway").await?;
    let mut gateway = LegacyGateway::new(
        "gateway",
        "identity",
        Some("valset".into()),
        bindings.identity_chain_id,
    );
    gateway
        .install(&bytes, root)
        .map_err(|error| format!("gateway install: {error}"))?;

    let (bytes, root) = snapshot_of("inbox").await?;
    let mut inbox = Inbox::new("inbox");
    inbox
        .install(&bytes, root)
        .map_err(|error| format!("inbox install: {error}"))?;

    let files_scratch = SyncScratch::prepare(duckfs_dir, attempt)
        .map_err(|error| format!("duckfs scratch: {error}"))?;
    let mut files = Files::open("files", files_scratch.dir().to_path_buf())
        .map_err(|error| format!("duckfs open: {error}"))?;
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
    .map_err(|error| format!("files sync: {error}"))?;

    let (bytes, root) = snapshot_of("jobs").await?;
    let mut jobs = Jobs::new("jobs");
    jobs.install(&bytes, root)
        .map_err(|error| format!("jobs install: {error}"))?;

    let (bytes, root) = snapshot_of("agent").await?;
    let mut agent = AgentModule::new("agent", "saga", Some("runs".into()))
        .with_legacy_v1_state();
    agent
        .install(&bytes, root)
        .map_err(|error| format!("agent install: {error}"))?;

    let (bytes, root) = snapshot_of("runs").await?;
    let mut runs = RunsModule::new(
        "runs",
        "chat",
        "saga",
        "tagging",
        "dispatch",
        "agent",
        Some("tasks".into()),
        Some("jobs".into()),
    )
    .with_files_module("files")
    .with_sink_forge("forge")
    .with_pages_module("pages");
    runs.install(&bytes, root)
        .map_err(|error| format!("runs install: {error}"))?;

    let (bytes, root) = snapshot_of("automations").await?;
    let mut automations = Automations::new("automations", "chat", "tasks", "inbox");
    automations
        .install(&bytes, root)
        .map_err(|error| format!("automations install: {error}"))?;

    let (bytes, root) = snapshot_of("forge").await?;
    let mut forge = Forge::with_blobs("forge", forge_repo.to_path_buf(), blobs)
        .map_err(|error| format!("forge init: {error}"))?
        .with_chat("chat");
    forge
        .install(&bytes, root)
        .map_err(|error| format!("forge install: {error}"))?;

    let mut host = NativeV1Modules {
        kv,
        pages,
        chat,
        forge,
        valset,
        clients,
        governance,
        upgrade,
        modreg,
        hello,
        saga,
        capability,
        dispatch,
        tagging,
        tasks,
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
    .map_err(|error| format!("compose native v1 synced host: {error}"))?;

    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "native v1 composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
        ));
    }

    files_scratch
        .promote(files_root.0)
        .map_err(|error| format!("duckfs promote: {error}"))?;
    host.register(Box::new(
        Files::open("files", duckfs_dir.to_path_buf())
            .map_err(|error| format!("duckfs reopen: {error}"))?,
    ));
    host.set_active_version(manifest.current_version);
    if host.app_hash() != manifest.app_hash {
        return Err(format!(
            "native v1 canonical duckfs reopen composed {} != manifest {}",
            hex(&host.app_hash()),
            hex(&manifest.app_hash)
        ));
    }
    Ok(host)
}
