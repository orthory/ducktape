//! the in-process package testbed: the standard platform module set under a
//! real [`Host`], block-by-block submission with explicit origins, the
//! canned-oracle seam, and the snapshot round-trip sweep.

use std::collections::{BTreeMap, VecDeque};
use std::future::Future;

use agent::AgentModule;
use capability::CapabilityRegistry;
use chat::{Chat, ChatMsg, PostPolicy, encode_msg as chat_encode_msg};
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use dispatch::{DispatchModule, decode_work_spec};
use host::{BlockContext, BlockOutcome, FinalizedBlock, Host, SubmitError};
use jobs::{JobStatus, Jobs, JobsQuery, JobsReply};
use memory::Memory;
use package::PackageModule;
use runs::{
    RunsModule, RunsMsg, RunsQuery, RunsReply, encode_msg as runs_encode_msg,
    encode_query as runs_encode_query,
};
use saga::{SagaModule, SagaMsg, decode_worker_request, encode_msg as saga_encode_msg};
use sdk::{Effect, Error, Module, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use tagging::TaggingModule;
use tasks::Tasks;

use crate::install::{InstallReport, build_report, install_spec_from_capsule};

/// the framework's own external submitter — the origin of the genesis
/// worker-enable block and of [`PackageTestBed::deliver`]'s benign blocks.
const DRIVER_KEY: &[u8] = b"quack-harness-driver";
/// the oracle's external origin — mirrors `collaboration_loop.rs`.
const ORACLE_KEY: &[u8] = b"oracle";

/// the builtin action routes the real genesis seeds into the package module.
fn builtin_routes() -> Vec<(String, String)> {
    vec![
        ("tasks.create".into(), "tasks".into()),
        ("tasks.update_status".into(), "tasks".into()),
    ]
}

/// one observability event of the most recently committed block, with its
/// payload rendered as text — what `expect_failure_row` matches against.
#[derive(Clone, Debug)]
pub struct BlockEvent {
    pub source: String,
    pub text: String,
}

/// how one module's committed state was proven to round-trip.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RoundtripKind {
    /// snapshot bytes were installed into a FRESH instance of the platform
    /// module type and the fresh `root()` reproduced the captured root.
    Reinstalled,
    /// a caller-supplied module's snapshot bytes were verified as the exact
    /// `root()` preimage (`sha256(bytes) == root`) — the platform's
    /// snapshot-bytes convention. full re-instantiation needs the concrete
    /// type, which only the package author's own suite holds; a module whose
    /// root is NOT the sha256 of its snapshot bytes fails this check and must
    /// prove itself in its own `snapshot_round_trip.rs`.
    PreimageVerified,
    /// a resolver-backed (qmdb) module's served sync target was verified to
    /// commit to the captured root. byte-level op-range replay is exercised
    /// by the module's own sync suite, not re-implemented here.
    ResolverVerified,
    /// no durable state to transfer; nothing to verify.
    Stateless,
}

/// one row of [`PackageTestBed::snapshot_roundtrip_all`]'s sweep.
#[derive(Clone, Debug)]
pub struct ModuleRoundtrip {
    pub id: String,
    pub root: StateRoot,
    pub kind: RoundtripKind,
}

/// an in-process `Host` with the standard platform set plus the caller's
/// package modules, driven block by block. heights auto-increment (one block
/// per submission, `consensus_time == height`) so scripts are deterministic
/// by construction — no wall clock, no randomness.
pub struct PackageTestBed {
    host: Host,
    next_height: u64,
    last_height: u64,
    noop_seq: u64,
    /// un-answered `WorkerRequest` effects, FIFO — the off-consensus seam the
    /// canned oracle answers through ordinary ops.
    pending_effects: VecDeque<Effect>,
    /// the events of the most recently committed block.
    last_events: Vec<BlockEvent>,
}

impl PackageTestBed {
    /// boot the platform set plus `package_modules` under the deterministic
    /// runtime and run `scenario` against the testbed — the one-call entry a
    /// package author's test uses.
    pub fn run<F, Fut, T>(package_modules: Vec<Box<dyn Module>>, scenario: F) -> T
    where
        F: FnOnce(PackageTestBed) -> Fut,
        Fut: Future<Output = T>,
    {
        deterministic::Runner::default().start(|context| async move {
            let bed = PackageTestBed::genesis(context, package_modules).await;
            scenario(bed).await
        })
    }

    /// assemble the testbed against a caller-provided deterministic context
    /// (use [`PackageTestBed::run`] unless the test already owns a runner).
    ///
    /// the platform set and its wiring mirror the real genesis
    /// (`collaboration_loop.rs` / `bin/noded`): chat reports to tagging, the
    /// agent registry hooks runs, runs routes actions through the package
    /// module, and the package module publishes prompt seeds into memory.
    /// block 1 enables the runs module as the single jobs worker — the live
    /// network's baseline — so caller blocks start at height 2.
    pub async fn genesis(
        context: deterministic::Context,
        package_modules: Vec<Box<dyn Module>>,
    ) -> Self {
        let chat = Chat::init(context.child("chat"), "chat")
            .await
            .with_tagging("tagging");
        let pages = pages::Pages::init(context.child("pages"), "pages").await;
        let mut modules: Vec<Box<dyn Module>> = vec![
            Box::new(chat),
            Box::new(pages),
            Box::new(TaggingModule::new("tagging")),
            Box::new(SagaModule::new("saga")),
            Box::new(DispatchModule::new("dispatch", "saga")),
            Box::new(AgentModule::new("agent", "saga", Some("runs".into()))),
            Box::new(RunsModule::new(
                "runs",
                "chat",
                "saga",
                "tagging",
                "dispatch",
                "agent",
                "package",
                Some("jobs".into()),
            )),
            Box::new(Tasks::new("tasks")),
            Box::new(Jobs::new("jobs")),
            Box::new(Memory::new("memory", "files")),
            Box::new(CapabilityRegistry::new("capability", None)),
            Box::new(PackageModule::new("package", "memory", builtin_routes())),
        ];
        modules.extend(package_modules);
        let host = Host::genesis(modules).expect("testbed genesis (duplicate module id?)");
        let mut bed = Self {
            host,
            next_height: 1,
            last_height: 0,
            noop_seq: 0,
            pending_effects: VecDeque::new(),
            last_events: Vec::new(),
        };
        bed.submit(
            bed.driver(),
            "runs",
            runs_encode_msg(&RunsMsg::EnableJobWorker { enabled: true }),
        )
        .await
        .expect("enable the runs module as the jobs worker");
        bed
    }

    /// the framework's default external submitter.
    pub fn driver(&self) -> Origin {
        Origin::External(DRIVER_KEY.to_vec())
    }

    pub fn host(&self) -> &Host {
        &self.host
    }

    pub fn app_hash(&self) -> StateRoot {
        self.host.app_hash()
    }

    /// the events of the most recently committed block.
    pub fn last_events(&self) -> &[BlockEvent] {
        &self.last_events
    }

    /// un-answered `WorkerRequest` effects awaiting an oracle turn.
    pub fn pending_oracle_requests(&self) -> usize {
        self.pending_effects.len()
    }

    /// submit one op as its own block. the height auto-increments per
    /// submission (rejected blocks consume their height too — rejection is
    /// deterministic, so replays stay aligned).
    pub async fn submit(
        &mut self,
        origin: Origin,
        target: &str,
        payload: Vec<u8>,
    ) -> Result<BlockOutcome, SubmitError> {
        let height = self.next_height;
        self.next_height += 1;
        let outcome = self
            .host
            .submit_at(
                BlockContext {
                    protocol_version: 0,
                    height,
                    consensus_time: height,
                    origin,
                },
                Msg {
                    target: target.into(),
                    payload,
                },
            )
            .await?;
        self.last_height = height;
        self.pending_effects.extend(outcome.effects.iter().cloned());
        self.last_events = outcome
            .events
            .iter()
            .map(|e| BlockEvent {
                source: e.source.clone(),
                text: String::from_utf8_lossy(&e.payload).into_owned(),
            })
            .collect();
        Ok(outcome)
    }

    /// submit a serde_json payload (the golden fixtures' `submit` step body).
    pub async fn submit_json(
        &mut self,
        origin: Origin,
        target: &str,
        payload: &serde_json::Value,
    ) -> Result<BlockOutcome, SubmitError> {
        let bytes = serde_json::to_vec(payload).expect("a json value serializes");
        self.submit(origin, target, bytes).await
    }

    /// external read-only query of a registered module.
    pub async fn query(&self, module: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.host.query(module, req).await
    }

    /// query with a serde_json request, decoding the reply as canonical JSON
    /// — the golden fixtures' `expect_query` lane.
    pub async fn query_json(
        &self,
        module: &str,
        query: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let req = serde_json::to_vec(query).expect("a json value serializes");
        let reply = self
            .query(module, &req)
            .await
            .map_err(|e| format!("query of {module} failed: {e}"))?;
        serde_json::from_slice(&reply)
            .map_err(|e| format!("{module} reply is not canonical JSON: {e}"))
    }

    /// answer the OLDEST pending `WorkerRequest` effect with a canned oracle
    /// outcome, submitted as an ordinary op (the N2 laundering seam): raw
    /// model bytes on `Ok`, a provider failure on `Err`.
    pub async fn oracle(
        &mut self,
        outcome: Result<Vec<u8>, String>,
    ) -> Result<BlockOutcome, String> {
        let effect = self
            .pending_effects
            .pop_front()
            .ok_or("no pending oracle request (no WorkerRequest effect outstanding)")?;
        let request = decode_worker_request(&effect.0)
            .map_err(|e| format!("effect is not a WorkerRequest: {e}"))?;
        // the kind gate a real worker applies: the spec must be a dispatch
        // WorkSpec (what the recipe promised the capability provider).
        decode_work_spec(&request.spec)
            .map_err(|e| format!("WorkerRequest spec is not a dispatch WorkSpec: {e}"))?;
        self.submit(
            Origin::External(ORACLE_KEY.to_vec()),
            "saga",
            saga_encode_msg(&SagaMsg::OracleResult {
                saga_id: request.saga_id,
                attempt: request.attempt,
                outcome,
            }),
        )
        .await
        .map_err(|e| format!("oracle block rejected: {e:?}"))
    }

    /// script one oracle turn whose raw model text is the compact encoding of
    /// `response` (a strict `AgentResponse`-shaped JSON value).
    pub async fn oracle_response_json(
        &mut self,
        response: &serde_json::Value,
    ) -> Result<BlockOutcome, String> {
        let raw = serde_json::to_vec(response).expect("a json value serializes");
        self.oracle(Ok(raw)).await
    }

    /// advance one block with a benign op — what triggers the host's
    /// committed delivery injection (the never-pop-stack rule's other half).
    /// the platform has no empty-block primitive, so this is a real chat
    /// `CreateChannel` from the framework driver, exactly like the
    /// `collaboration_loop.rs` noop blocks.
    pub async fn deliver(&mut self) -> Result<BlockOutcome, String> {
        self.noop_seq += 1;
        let channel = format!("quack-harness-noop-{}", self.noop_seq);
        self.submit(
            self.driver(),
            "chat",
            chat_encode_msg(&ChatMsg::CreateChannel {
                channel_id: channel,
                name: "Quack Harness Noop".into(),
                post_policy: PostPolicy::Open,
            }),
        )
        .await
        .map_err(|e| format!("delivery block rejected: {e:?}"))
    }

    /// build the install spec from `capsule` (see [`install_spec_from_capsule`]),
    /// pre-check the manifest's required platform modules against the
    /// registered set, submit `PackageMsg::Install` from `installer`, and
    /// report what actually landed.
    pub async fn install_capsule(
        &mut self,
        capsule: &quack::Capsule,
        harness_logical: &str,
        bindings: &BTreeMap<String, String>,
        installer: Origin,
    ) -> Result<InstallReport, String> {
        let spec = install_spec_from_capsule(capsule, harness_logical, bindings)?;
        let toml = capsule
            .manifest_bytes()
            .expect("mapping checked the manifest");
        let manifest = quack::parse_manifest(toml).expect("mapping parsed the manifest");
        for required in &manifest.requires.modules {
            if self.host.module_root(required).is_none() {
                return Err(format!(
                    "required module {required:?} is not registered on the testbed"
                ));
            }
        }
        // requires.capabilities name off-consensus provider tags; the testbed
        // has no live providers by design (the oracle is scripted), so they
        // are not checked here.
        self.submit(
            installer,
            "package",
            package::encode_msg(&package::PackageMsg::Install(spec.clone())),
        )
        .await
        .map_err(|e| format!("install rejected: {e:?}"))?;
        build_report(&self.host, &spec).await
    }

    /// install a package SOURCE DIRECTORY (`quack.toml` at its root).
    pub async fn install_dir(
        &mut self,
        dir: &std::path::Path,
        harness_logical: &str,
        bindings: &BTreeMap<String, String>,
        installer: Origin,
    ) -> Result<InstallReport, String> {
        let capsule = quack::open_dir(dir).map_err(|e| format!("open {dir:?}: {e}"))?;
        self.install_capsule(&capsule, harness_logical, bindings, installer)
            .await
    }

    // ---- the assertion kit (panicking, test-style) ---------------------------

    /// assert the jobs board holds exactly `expected` jobs of `kind_prefix`
    /// (any status) — "an engagement event mints exactly one job".
    pub async fn assert_job_count(&self, kind_prefix: &str, expected: usize) {
        let jobs = self.jobs_matching(kind_prefix).await;
        assert_eq!(
            jobs.len(),
            expected,
            "expected {expected} jobs with kind prefix {kind_prefix:?}, found {}: {:?}",
            jobs.len(),
            jobs.iter().map(|j| j.job_id.as_str()).collect::<Vec<_>>()
        );
    }

    /// assert one job's status.
    pub async fn assert_job_status(&self, job_id: &str, expected: JobStatus) {
        let reply = self
            .query(
                "jobs",
                &jobs::encode_query(&JobsQuery::Get {
                    job_id: job_id.into(),
                }),
            )
            .await
            .expect("jobs query");
        match jobs::decode_reply(&reply).expect("jobs reply") {
            JobsReply::Job(Some(job)) => assert_eq!(
                job.status, expected,
                "job {job_id} is {:?}, expected {expected:?}",
                job.status
            ),
            JobsReply::Job(None) => panic!("job {job_id} does not exist"),
            other => panic!("unexpected jobs reply: {other:?}"),
        }
    }

    /// assert whether a pending (not-yet-delivered) run exists for `agent_id`.
    pub async fn assert_pending_run_for_agent(&self, agent_id: &str, exists: bool) {
        let runs = self.pending_runs().await;
        let found = runs.iter().any(|r| r.agent_id == agent_id);
        assert_eq!(
            found,
            exists,
            "pending runs for agent {agent_id:?}: {:?}",
            runs.iter().map(|r| r.run_id.as_str()).collect::<Vec<_>>()
        );
    }

    /// assert the package registry resolves `tag` to `owner`.
    pub async fn assert_action_owner(&self, tag: &str, owner: Option<&str>) {
        let reply = self
            .query(
                "package",
                &package::encode_query(&package::PackageQuery::ActionOwner { tag: tag.into() }),
            )
            .await
            .expect("package query");
        match package::decode_reply(&reply).expect("package reply") {
            package::PackageReply::Owner(actual) => {
                assert_eq!(actual.as_deref(), owner, "action {tag:?} owner mismatch")
            }
            other => panic!("unexpected package reply: {other:?}"),
        }
    }

    /// assert the MOST RECENTLY COMMITTED block left a breadcrumb event from
    /// `source` containing `contains` — how the no-fail arms record failure
    /// ("mutate nothing, record failure").
    pub fn assert_failure_breadcrumb(&self, source: &str, contains: &str) {
        assert!(
            self.has_failure_breadcrumb(source, contains),
            "no event from {source:?} containing {contains:?} in the last block; events: {:?}",
            self.last_events
        );
    }

    pub(crate) fn has_failure_breadcrumb(&self, source: &str, contains: &str) -> bool {
        self.last_events
            .iter()
            .any(|e| e.source == source && e.text.contains(contains))
    }

    pub(crate) async fn jobs_matching(&self, kind_prefix: &str) -> Vec<jobs::Job> {
        let reply = self
            .query(
                "jobs",
                &jobs::encode_query(&JobsQuery::List {
                    status: None,
                    kind_prefix: kind_prefix.into(),
                    limit: 10_000,
                }),
            )
            .await
            .expect("jobs query");
        match jobs::decode_reply(&reply).expect("jobs reply") {
            JobsReply::Jobs(jobs) => jobs,
            other => panic!("unexpected jobs reply: {other:?}"),
        }
    }

    pub(crate) async fn pending_runs(&self) -> Vec<runs::PendingRun> {
        let reply = self
            .query("runs", &runs_encode_query(&RunsQuery::PendingRuns))
            .await
            .expect("runs query");
        match runs::decode_reply(&reply).expect("runs reply") {
            RunsReply::PendingRuns(runs) => runs,
            other => panic!("unexpected runs reply: {other:?}"),
        }
    }

    // ---- the snapshot round-trip sweep ---------------------------------------

    /// prove every registered module's committed state round-trips at the
    /// current boundary, per substrate kind (see [`RoundtripKind`] for what
    /// each kind honestly asserts). a module that declares NO state-sync
    /// surface fails the sweep — the ADR requires every module to reproduce
    /// its root from snapshots/state sync.
    pub async fn snapshot_roundtrip_all(&self) -> Result<Vec<ModuleRoundtrip>, String> {
        let snapshot = self
            .host
            .capture_finalized_snapshot(FinalizedBlock {
                height: self.last_height,
                app_hash: self.host.app_hash(),
            })
            .map_err(|e| format!("capture failed: {e}"))?;

        let mut report = Vec::new();
        for module in &snapshot.modules {
            let kind = match &module.state_sync {
                StateSyncHandle::SnapshotBytes(bytes) => {
                    match reinstall_platform(&module.id, bytes, module.root) {
                        Some(Ok(())) => RoundtripKind::Reinstalled,
                        Some(Err(e)) => {
                            return Err(format!(
                                "module {} failed to re-install its snapshot into a fresh \
                                 instance: {e}",
                                module.id
                            ));
                        }
                        None => {
                            // a caller-supplied module: verify the platform
                            // snapshot-bytes convention — the bytes are the
                            // exact root preimage (what a joiner's install
                            // verifies before adopting).
                            let digest = StateRoot(Sha256::digest(bytes).into());
                            if digest != module.root {
                                return Err(format!(
                                    "module {}: snapshot bytes do not hash to root(): the \
                                     framework can only preimage-verify caller-supplied \
                                     snapshot-bytes modules (sha256(snapshot) == root, the \
                                     memory/tasks/package convention); a module with a \
                                     different root derivation must prove its round-trip in \
                                     its own snapshot suite",
                                    module.id
                                ));
                            }
                            RoundtripKind::PreimageVerified
                        }
                    }
                }
                StateSyncHandle::ResolverBacked { .. } => {
                    let target = self
                        .host
                        .resolver_sync_target(&module.id)
                        .await
                        .map_err(|e| format!("module {}: resolver target: {e}", module.id))?;
                    if target.root != module.root {
                        return Err(format!(
                            "module {}: served sync target root does not match the committed \
                             root",
                            module.id
                        ));
                    }
                    RoundtripKind::ResolverVerified
                }
                StateSyncHandle::Stateless => RoundtripKind::Stateless,
                StateSyncHandle::Unsupported { reason } => {
                    return Err(format!(
                        "module {} declares no state-sync surface ({reason}) — the ADR requires \
                         every module's snapshots/state sync to reproduce its root",
                        module.id
                    ));
                }
            };
            report.push(ModuleRoundtrip {
                id: module.id.clone(),
                root: module.root,
                kind,
            });
        }
        Ok(report)
    }
}

/// install `bytes` into a FRESH instance of the platform module registered
/// under `id`, verifying it lands on `root`. `None` for non-platform ids —
/// the concrete type (and its `install`) lives outside this crate.
///
/// constructor args mirror [`PackageTestBed::genesis`]; none of them enter
/// any module's root preimage, so the fresh instance's identity is exactly
/// the snapshot's.
fn reinstall_platform(id: &str, bytes: &[u8], root: StateRoot) -> Option<Result<(), String>> {
    fn check<M: Module + InstallableSnapshot>(
        mut fresh: M,
        bytes: &[u8],
        root: StateRoot,
    ) -> Result<(), String> {
        fresh
            .install_snapshot(bytes, root)
            .map_err(|e| e.to_string())?;
        if fresh.root() != root {
            return Err("fresh instance root diverged after install".into());
        }
        Ok(())
    }

    Some(match id {
        "tagging" => check(TaggingModule::new("tagging"), bytes, root),
        "saga" => check(SagaModule::new("saga"), bytes, root),
        "dispatch" => check(DispatchModule::new("dispatch", "saga"), bytes, root),
        "agent" => check(
            AgentModule::new("agent", "saga", Some("runs".into())),
            bytes,
            root,
        ),
        "runs" => check(
            RunsModule::new(
                "runs",
                "chat",
                "saga",
                "tagging",
                "dispatch",
                "agent",
                "package",
                Some("jobs".into()),
            ),
            bytes,
            root,
        ),
        "tasks" => check(Tasks::new("tasks"), bytes, root),
        "jobs" => check(Jobs::new("jobs"), bytes, root),
        "memory" => check(Memory::new("memory", "files"), bytes, root),
        "capability" => check(CapabilityRegistry::new("capability", None), bytes, root),
        "package" => check(
            PackageModule::new("package", "memory", builtin_routes()),
            bytes,
            root,
        ),
        _ => return None,
    })
}

/// the platform modules' shared snapshot-install surface (each exposes an
/// inherent `install(bytes, expected_root)`); unified here so the sweep's
/// fresh-instance check is one generic path.
trait InstallableSnapshot {
    fn install_snapshot(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error>;
}

macro_rules! installable {
    ($($ty:ty),+ $(,)?) => {$(
        impl InstallableSnapshot for $ty {
            fn install_snapshot(&mut self, bytes: &[u8], expected: StateRoot) -> Result<(), Error> {
                self.install(bytes, expected)
            }
        }
    )+};
}

installable!(
    TaggingModule,
    SagaModule,
    DispatchModule,
    AgentModule,
    RunsModule,
    Tasks,
    Jobs,
    Memory,
    CapabilityRegistry,
    PackageModule,
);
