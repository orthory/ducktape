//! the in-process package testbed: the standard platform module set under a
//! real [`Host`], block-by-block submission with explicit origins, and the
//! install driving seam. the canned-oracle seam lives in [`oracle`], the
//! panicking assertion kit in [`assertions`], and the snapshot round-trip
//! sweep in [`snapshot`] — each its own module, this file owns only
//! construction (`run`/`genesis`) and the core submit/query/install surface.

use std::collections::{BTreeMap, VecDeque};
use std::future::Future;

use agent::AgentModule;
use capability::CapabilityRegistry;
use chat::Chat;
use commonware_runtime::{Runner as _, Supervisor as _, deterministic};
use dispatch::DispatchModule;
use host::{BlockContext, BlockOutcome, Host};
use jobs::Jobs;
use memory::Memory;
use package::PackageModule;
use runs::{RunsModule, RunsMsg, encode_msg as runs_encode_msg};
use saga::SagaModule;
use sdk::{Effect, Error, Module, Msg, Origin, StateRoot};
use tagging::TaggingModule;
use tasks::Tasks;

use crate::error::HarnessError;
use crate::install::{InstallReport, build_install_spec, build_report, parse_verified_manifest};

mod assertions;
mod oracle;
mod snapshot;

pub use snapshot::{ModuleRoundtrip, RoundtripKind};

/// the framework's own external submitter — the origin of the genesis
/// worker-enable block and of [`PackageTestBed::deliver`]'s benign blocks.
const DRIVER_KEY: &[u8] = b"quack-harness-driver";

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

    /// submit one op as its own block, tagging a submit rejection with
    /// `context` (what the caller was trying to do — "install rejected",
    /// "oracle block rejected", ...). the height auto-increments per
    /// submission (rejected blocks consume their height too — rejection is
    /// deterministic, so replays stay aligned).
    async fn submit_with_context(
        &mut self,
        context: &'static str,
        origin: Origin,
        target: &str,
        payload: Vec<u8>,
    ) -> Result<BlockOutcome, HarnessError> {
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
            .await
            .map_err(|source| HarnessError::Submit { context, source })?;
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

    /// submit one op as its own block. the height auto-increments per
    /// submission (rejected blocks consume their height too — rejection is
    /// deterministic, so replays stay aligned).
    pub async fn submit(
        &mut self,
        origin: Origin,
        target: &str,
        payload: Vec<u8>,
    ) -> Result<BlockOutcome, HarnessError> {
        self.submit_with_context("submit", origin, target, payload)
            .await
    }

    /// submit a serde_json payload (the golden fixtures' `submit` step body).
    pub async fn submit_json(
        &mut self,
        origin: Origin,
        target: &str,
        payload: &serde_json::Value,
    ) -> Result<BlockOutcome, HarnessError> {
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
    ) -> Result<serde_json::Value, HarnessError> {
        let req = serde_json::to_vec(query).expect("a json value serializes");
        let reply = self
            .query(module, &req)
            .await
            .map_err(|e| HarnessError::JsonQuery {
                module: module.to_string(),
                reason: e.to_string(),
            })?;
        serde_json::from_slice(&reply).map_err(|e| HarnessError::NotCanonicalJson {
            module: module.to_string(),
            reason: e.to_string(),
        })
    }

    /// build the install spec from `capsule` (see
    /// [`crate::install_spec_from_capsule`]), pre-check the manifest's
    /// required platform modules against the registered set, submit
    /// `PackageMsg::Install` from `installer`, and report what actually
    /// landed.
    pub async fn install_capsule(
        &mut self,
        capsule: &quack::Capsule,
        harness_logical: &str,
        bindings: &BTreeMap<String, String>,
        installer: Origin,
    ) -> Result<InstallReport, HarnessError> {
        // ONE parse: build_install_spec reuses the manifest parse_verified_manifest
        // already derives, instead of separately re-deriving the InstallSpec
        // from scratch AND a second bare parse just for `requires.modules`.
        let manifest = parse_verified_manifest(capsule)?;
        let spec = build_install_spec(capsule, &manifest, Some(harness_logical), bindings)?;
        self.install_prepared(&manifest, spec, installer).await
    }

    /// install using an ALREADY-BUILT spec derived from an ALREADY-parsed
    /// manifest — the golden `Install` step's seam: it resolves the spec once
    /// (to check the fixture's package-id invariant) and must not force this
    /// method to re-derive it from the capsule bytes.
    pub(crate) async fn install_prepared(
        &mut self,
        manifest: &quack::PackageManifest,
        spec: package::InstallSpec,
        installer: Origin,
    ) -> Result<InstallReport, HarnessError> {
        for required in &manifest.requires.modules {
            if self.host.module_root(required).is_none() {
                return Err(HarnessError::MissingRequiredModule {
                    module: required.clone(),
                });
            }
        }
        // requires.capabilities name off-consensus provider tags; the testbed
        // has no live providers by design (the oracle is scripted), so they
        // are not checked here.
        self.submit_with_context(
            "install rejected",
            installer,
            "package",
            package::encode_msg(&package::PackageMsg::Install(spec.clone())),
        )
        .await?;
        build_report(&self.host, &spec).await
    }

    /// install a package SOURCE DIRECTORY (`quack.toml` at its root).
    pub async fn install_dir(
        &mut self,
        dir: &std::path::Path,
        harness_logical: &str,
        bindings: &BTreeMap<String, String>,
        installer: Origin,
    ) -> Result<InstallReport, HarnessError> {
        let capsule = quack::open_dir(dir).map_err(|source| HarnessError::OpenDir {
            path: format!("{dir:?}"),
            source,
        })?;
        self.install_capsule(&capsule, harness_logical, bindings, installer)
            .await
    }
}
