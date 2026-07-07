//! the snapshot round-trip sweep: prove every registered module's committed
//! state reproduces its root from snapshots/state sync, per substrate kind.

use agent::AgentModule;
use capability::CapabilityRegistry;
use dispatch::DispatchModule;
use host::FinalizedBlock;
use jobs::Jobs;
use memory::Memory;
use package::PackageModule;
use runs::RunsModule;
use saga::SagaModule;
use sdk::{Error, Module, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use tagging::TaggingModule;
use tasks::Tasks;

use super::{PackageTestBed, builtin_routes};
use crate::error::HarnessError;

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

impl PackageTestBed {
    // ---- the snapshot round-trip sweep ---------------------------------------

    /// prove every registered module's committed state round-trips at the
    /// current boundary, per substrate kind (see [`RoundtripKind`] for what
    /// each kind honestly asserts). a module that declares NO state-sync
    /// surface fails the sweep — the ADR requires every module to reproduce
    /// its root from snapshots/state sync.
    pub async fn snapshot_roundtrip_all(&self) -> Result<Vec<ModuleRoundtrip>, HarnessError> {
        let snapshot = self
            .host
            .capture_finalized_snapshot(FinalizedBlock {
                height: self.last_height,
                app_hash: self.host.app_hash(),
            })
            .map_err(|e| HarnessError::CaptureFailed(e.to_string()))?;

        let mut report = Vec::new();
        for module in &snapshot.modules {
            let kind = match &module.state_sync {
                StateSyncHandle::SnapshotBytes(bytes) => {
                    match reinstall_platform(&module.id, bytes, module.root) {
                        Some(Ok(())) => RoundtripKind::Reinstalled,
                        Some(Err(reason)) => {
                            return Err(HarnessError::ReinstallFailed {
                                module: module.id.clone(),
                                reason,
                            });
                        }
                        None => {
                            // a caller-supplied module: verify the platform
                            // snapshot-bytes convention — the bytes are the
                            // exact root preimage (what a joiner's install
                            // verifies before adopting).
                            let digest = StateRoot(Sha256::digest(bytes).into());
                            if digest != module.root {
                                return Err(HarnessError::NotPreimageVerified {
                                    module: module.id.clone(),
                                });
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
                        .map_err(|e| HarnessError::ResolverTarget {
                            module: module.id.clone(),
                            reason: e.to_string(),
                        })?;
                    if target.root != module.root {
                        return Err(HarnessError::ResolverRootMismatch {
                            module: module.id.clone(),
                        });
                    }
                    RoundtripKind::ResolverVerified
                }
                StateSyncHandle::Stateless => RoundtripKind::Stateless,
                StateSyncHandle::Unsupported { reason } => {
                    return Err(HarnessError::NoStateSyncSurface {
                        module: module.id.clone(),
                        reason: reason.clone(),
                    });
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
