//! the module interface crate — the only universal dependency for modules.
//!
//! a super-app feature (documents, forge, chat, tasks, …) is an isolated module:
//! a crate that implements [`Module`] and normally depends on `sdk` plus
//! types-only interface crates for any modules it talks to. a narrow wrapper
//! exception exists for wrapper modules that reuse a shared storage
//! implementation. in embedded mode, all durable state must move through the
//! wrapper's [`StateRoot`], commit/abort, and sync boundary. in facade mode, the
//! storage implementation is an explicitly registered backing module and durable
//! state belongs to that backing module's root. the host composes each module's
//! [`StateRoot`] into the global app-hash (see `host::global_root`); how a module
//! *computes* that root — a qmdb merkle root, a git HEAD oid — is private to the
//! module. the host only ever sees `root() -> StateRoot`.
//!
//! this crate also carries the deterministic *system api*: the [`Ctx`] a module
//! touches during state-machine application (own-state r/w lives in `self`;
//! read-only cross-module [`Ctx::query`]/[`Ctx::module_root`]; the deterministic
//! [`Env`]; and intent emission via [`Ctx::emit_msg`]/[`Ctx::emit_event`]/
//! [`Ctx::request_effect`]). the effectful node surface (real network/IO) is a
//! separate layer and out of scope here.
//!
//! keep this crate types + traits with no domain deps (async-trait is the one
//! greenlit exception): everything here is a shared surface for every module.
//! [`codec`] carries the shared zero-dep snapshot-codec primitives on the same
//! everyone-needs-it grounds.

pub mod codec;

/// length of an authenticated state root, in bytes. both substrates we use emit
/// 32-byte digests — a qmdb merkle root and a sha256-mode git oid — so a module
/// root is substrate-agnostic at exactly this width.
pub const ROOT_LEN: usize = 32;

/// a module's authenticated commitment to its entire state: a qmdb merkle root,
/// or forge's git HEAD oid. opaque to the host; only compared and re-hashed.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateRoot(pub [u8; ROOT_LEN]);

impl StateRoot {
    /// the root of an empty / uninitialized module.
    pub const ZERO: StateRoot = StateRoot([0u8; ROOT_LEN]);

    pub const fn as_bytes(&self) -> &[u8; ROOT_LEN] {
        &self.0
    }
}

impl core::fmt::Debug for StateRoot {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "StateRoot(")?;
        for b in &self.0 {
            write!(f, "{b:02x}")?;
        }
        write!(f, ")")
    }
}

/// how a module can serve its committed state at a block boundary.
///
/// The host uses this as an honesty surface for snapshot orchestration: raw
/// bytes are installable by modules that already expose snapshot/install, while
/// resolver-backed modules explicitly report that a caller must use their
/// module-specific sync target and resolver instead of pretending the host has a
/// byte snapshot for them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateSyncHandle {
    /// no durable module state needs transfer; recreate the module at genesis.
    Stateless,
    /// self-contained bytes that can be installed against the module root.
    SnapshotBytes(Vec<u8>),
    /// sync is available, but only through a module-specific resolver path.
    ResolverBacked {
        /// storage/sync backend name, e.g. "qmdb".
        backend: String,
        /// short operator-facing note describing the required handle.
        detail: String,
    },
    /// this module has not declared a state-sync surface.
    Unsupported {
        /// why the host cannot serve or describe a sync handle for this module.
        reason: String,
    },
}

impl StateSyncHandle {
    pub fn has_snapshot_bytes(&self) -> bool {
        matches!(self, Self::SnapshotBytes(_))
    }

    pub fn is_self_contained(&self) -> bool {
        matches!(self, Self::Stateless | Self::SnapshotBytes(_))
    }
}

/// a module's stable identity within the app. assigned at genesis and part of
/// consensus state — NOT per-node config — so every validator composes the same
/// global root in the same order.
pub type ModuleId = String;

// ============================================================================
// protocol-version preflight — a serializable mirror of the upgrade module's
// pending coordinates plus the pure boot check. these types live here so the
// recovery and state-sync manifest crates can carry a version HINT without
// depending on the upgrade module: the authoritative version stays derivable
// from the replayed/committed upgrade-module state and is confirmed by the
// final app-hash compose, so a lying manifest can at worst mis-preflight a
// node, never induce a fork.
// ============================================================================

/// the coordinates of a scheduled upgrade, mirrored for the manifests. shape
/// matches `upgrade::ScheduledUpgrade` but carries no module dependency.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeCoords {
    pub name: String,
    pub activation_height: u64,
    pub to_version: u32,
}

/// a boot-preflight refusal: the local build is too old to safely apply the
/// blocks at or after a boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedVersion {
    /// the highest protocol version any block at/after the boundary needs.
    pub required_min: u32,
    /// the highest protocol version the local build can apply
    /// (`MAX_PROTOCOL_VERSION`).
    pub max_supported: u32,
}

impl core::fmt::Display for UnsupportedVersion {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "this boundary needs protocol v{}; binary supports up to v{} — install the newer node binary",
            self.required_min, self.max_supported
        )
    }
}

impl std::error::Error for UnsupportedVersion {}

/// pure boot preflight: fail loud when the local build's `max_supported`
/// protocol version cannot serve a boundary requiring `required_min`. the
/// authority stays the app-hash; this only turns an opaque post-replay
/// app-hash mismatch into an early "height needs binary vX" refusal.
pub fn check_required_version(
    required_min: u32,
    max_supported: u32,
) -> Result<(), UnsupportedVersion> {
    if max_supported < required_min {
        Err(UnsupportedVersion {
            required_min,
            max_supported,
        })
    } else {
        Ok(())
    }
}

/// the highest protocol version any block at or after a boundary needs:
/// `to_version` once the served height has reached a pending upgrade's
/// activation, else the boundary's `current_version`. shared so the recovery
/// capture and the state-sync server derive the same fence.
pub fn required_min_version(
    current_version: u32,
    pending: Option<&UpgradeCoords>,
    height: u64,
) -> u32 {
    match pending {
        Some(u) if height >= u.activation_height => u.to_version,
        _ => current_version,
    }
}

// ============================================================================
// the deterministic system api — envelopes, env, error, ctx, module seam.
// ============================================================================

/// a write intent at another module (or self). emitted via [`Ctx::emit_msg`] and
/// re-dispatched by the host as a FOLLOW-UP op after the current `execute`
/// returns — never a reentrant mutating call. payload bytes are typed later via
/// per-module crate-root wire types; the host treats them opaquely.
#[derive(Clone, Debug)]
pub struct Msg {
    pub target: ModuleId,
    pub payload: Vec<u8>,
}

/// an observability record a module emits via [`Ctx::emit_event`]. it LEAVES the
/// state machine (handed to the effectful node layer) and never re-enters as a
/// follow-up.
#[derive(Clone, Debug)]
pub struct Event {
    pub source: ModuleId,
    pub payload: Vec<u8>,
}

/// a request for an effectful, non-deterministic side effect (data channel,
/// tunnel, transport upgrade). STUB this slice: the host only collects it.
#[derive(Clone, Debug)]
pub struct Effect(pub Vec<u8>);

/// who triggered the current dispatch. varies across follow-ups: the root op is
/// `External`/`System`; an emitted follow-up is `Module(emitter_id)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Origin {
    /// an external submitter, identified by (e.g.) an ed25519 id.
    External(Vec<u8>),
    /// a module that emitted this as a follow-up.
    Module(ModuleId),
    /// genesis / system-internal.
    System,
}

/// the deterministic environment handed to `execute`. block-constant fields
/// (`height`, `consensus_time`) are identical across every dispatch in one
/// `submit`; `origin` and `me` vary per dispatch. NOT wall clock, NOT per-node.
#[derive(Clone, Debug)]
pub struct Env {
    /// block / consensus round.
    pub height: u64,
    /// agreed timestamp — NOT wall clock.
    pub consensus_time: u64,
    /// who triggered THIS dispatch.
    pub origin: Origin,
    /// the module being dispatched.
    pub me: ModuleId,
    /// the effective protocol version for this block — a verbatim copy of
    /// `BlockContext.protocol_version`, stamped by the host drain and identical
    /// across the root op and every FIFO follow-up in one `submit`. this is the
    /// ONLY version signal a module may branch on inside `execute`/`query`; it is
    /// a read-only dispatch input and is NEVER folded into any `root()` preimage
    /// or op/wire encoding. defaults to the baseline version (`0`).
    pub protocol_version: u32,
}

/// a resolver-backed module's committed sync target at one boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolverSyncTarget {
    pub root: StateRoot,
    pub start: u64,
    pub op_count: u64,
}

/// errors surfaced through the system api.
#[derive(Clone, PartialEq, Eq)]
pub enum Error {
    /// dispatch / query targeted a module that is not registered.
    UnknownModule(ModuleId),
    /// `ctx.query(env.me, ..)` — read your own state directly via `self`.
    SelfQuery,
    /// a module has no sync read projection (the default `Module::query`).
    QueryUnsupported,
    /// a module has no byte-level state-sync serve surface (the default
    /// [`Module::serve_sync`]).
    SyncUnsupported,
    /// the local follow-up drain exceeded its dispatch budget (non-termination
    /// guard).
    BudgetExceeded,
    /// bubbled out of a module's `execute`/`query`.
    Module(String),
}

impl core::fmt::Debug for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Error::UnknownModule(id) => write!(f, "UnknownModule({id})"),
            Error::SelfQuery => write!(f, "SelfQuery"),
            Error::QueryUnsupported => write!(f, "QueryUnsupported"),
            Error::SyncUnsupported => write!(f, "SyncUnsupported"),
            Error::BudgetExceeded => write!(f, "BudgetExceeded"),
            Error::Module(m) => write!(f, "Module({m})"),
        }
    }
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        core::fmt::Debug::fmt(self, f)
    }
}

impl std::error::Error for Error {}

/// the deterministic surface a module touches during state-machine application.
/// object-safe via async-trait (its one async method is boxed) so it can be passed as
/// `&mut dyn Ctx`: own-state r/w is private to `self`; cross-module reads are
/// sync and host-routed; writes are emitted as intents, never reentrant calls.
#[async_trait::async_trait(?Send)]
pub trait Ctx {
    /// the deterministic environment for this dispatch.
    fn env(&self) -> &Env;

    /// SNAPSHOT root of `target` as of the START of this dispatch (self
    /// included). NOT live — does not reflect mutations made during the current
    /// `execute`. a module's own live root is `self.root()`.
    fn module_root(&self, target: &str) -> Option<StateRoot>;

    /// live, read-only, host-routed read of another module. `target == env.me`
    /// is rejected with [`Error::SelfQuery`]. the host routes this to
    /// [`Module::query_with`] (whose default delegates to [`Module::query`]) —
    /// filtered facade modules depend on receiving the `query_with` ctx.
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error>;

    /// emit a write intent — collected, re-dispatched as a follow-up op; never
    /// executed reentrantly.
    fn emit_msg(&mut self, msg: Msg);

    /// emit an observability event — leaves the state machine.
    fn emit_event(&mut self, ev: Event);

    /// request an effectful side effect — STUB this slice (collected only).
    fn request_effect(&mut self, eff: Effect);
}

/// the host-facing surface of a feature module: identity, authenticated root, the
/// async dispatch entry point, and a read-only query projection.
///
/// `#[async_trait(?Send)]`: `execute` is awaited inline by the host's dispatch
/// loop, never spawned onto a separate task, so its future need not be `Send` —
/// and the host's `Ctx` borrows the rest of the registry across the await (for
/// `query` routing), which would make a `Send` future impossible anyway.
#[async_trait::async_trait(?Send)]
pub trait Module {
    /// this module's genesis-assigned id (e.g. "documents", "forge").
    fn id(&self) -> ModuleId;

    /// the module's current authenticated root. called by the host to fold into
    /// the global app-hash after a block applies.
    fn root(&self) -> StateRoot;

    /// describe the committed-state sync surface for this module.
    ///
    /// This is called by snapshot orchestration after the host has reached a
    /// block boundary. The default is explicit non-coverage; modules that expose
    /// installable snapshot bytes or a resolver-backed sync path should override
    /// it so a live node can advertise exactly what it can serve.
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        Ok(StateSyncHandle::Unsupported {
            reason: "module did not declare a state-sync handle".into(),
        })
    }

    /// serve one byte-level state-sync request against COMMITTED state.
    ///
    /// this is the routable serve surface behind [`StateSyncHandle::
    /// ResolverBacked`]: a running node holds its modules as `dyn Module` in the
    /// host registry, so a network state-sync service can only reach a module's
    /// sync backend through the trait. request/response bytes are module-defined
    /// (a qmdb-backed module answers sync-target and proof-carrying op-range
    /// requests; the shared wire shapes live in the kernel `statesync` crate) —
    /// the host and transport treat them opaquely, exactly like query bytes.
    ///
    /// read-only by contract: serving MUST NOT mutate module state, and MUST be
    /// answered from committed state only (never a staged overlay) — the host
    /// routes it outside any block, and responses are verified by the CALLER
    /// against a consensus-committed root, so a dishonest response can never
    /// install. the default is explicit non-coverage for modules whose whole
    /// sync surface is [`StateSyncHandle::SnapshotBytes`].
    async fn serve_sync(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::SyncUnsupported)
    }

    /// return the committed resolver target for modules that advertise
    /// [`StateSyncHandle::ResolverBacked`]. default modules have no resolver
    /// lane, so they cannot provide a target.
    async fn resolver_sync_target(&self) -> Result<ResolverSyncTarget, Error> {
        Err(Error::SyncUnsupported)
    }

    /// the dispatch entry point. async, but every `.await` MUST be on a
    /// deterministic resource (own qmdb state, a query) — NEVER a network/effect.
    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error>;

    /// read-only projection serving other modules' [`Ctx::query`]. async, so a
    /// qmdb-backed module can serve a real read (`self.db.get(..).await`). defaults
    /// to [`Error::QueryUnsupported`] for modules with no read path.
    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }

    /// read-only projection with access to host-routed reads of sibling modules.
    /// filtered facade modules can override this to present another module's
    /// durable state without copying it. standalone modules can keep implementing
    /// [`Module::query`]; the default delegates there.
    async fn query_with(&self, _ctx: &dyn Ctx, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.query(req).await
    }

    /// BLOCK-BOUNDARY COMMIT. a module STAGES its writes during `execute` and
    /// publishes them here, once, when the host declares the block a success.
    /// after this returns, `root()` MUST reflect the staged writes. the default
    /// is a no-op — for stateless modules and any module that has no staging
    /// seam. called by the host in deterministic (registry) order over exactly
    /// the modules dispatched this block.
    async fn commit_block(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// BLOCK-BOUNDARY ABORT. the host calls this on EVERY module it dispatched
    /// when the block fails partway (a later `execute` errored, or the dispatch
    /// budget was exceeded): the module MUST discard its staged writes so the
    /// block leaves no trace — `root()` is byte-identical to its pre-block value.
    /// the default is a no-op.
    async fn abort_block(&mut self) -> Result<(), Error> {
        Ok(())
    }

    /// the PER-COMMIT HEIGHT CURSOR of a per-block-durable (disk-cohort)
    /// module: the block height of its most recent durable commit, persisted
    /// ATOMICALLY with that commit — inside the same fsync'd durability unit
    /// as the module's own state advance (e.g. the duckfs refs-file envelope,
    /// or a qmdb commit-metadata slot) — or `None` when the module tracks no
    /// cursor (the default: the in-memory cohort and legacy disk modules).
    ///
    /// recovery consults this to BOUND-AND-VERIFY a trailing durable commit
    /// whose journal seal was lost to a power cut: a disk module whose live
    /// root matches no recorded post-root is accepted ONLY when its cursor
    /// claims exactly the journal's single unsealed WAL height; any other
    /// value (or no cursor) stays fail-closed as torn state. the atomicity
    /// requirement is load-bearing — a cursor written in a separate
    /// durability step could survive while the state write it describes was
    /// lost (or vice versa), turning the bound into a lie.
    ///
    /// NEVER a consensus input: the height lives outside every `root()`
    /// preimage and outside all op/wire encodings (per-node recovery
    /// bookkeeping only).
    fn durable_commit_height(&self) -> Option<u64> {
        None
    }

    /// ACTIVATION HOOK. the host drives this across the whole registry at the
    /// finalized activation boundary (from the orchestrator's agreed
    /// `RespawnPlan::boundary_version`) so a `root()`-changing dual-path module
    /// selects its NEW branch deterministically at `H` (design §4). `version` is
    /// the effective boundary protocol version — an agreed, non-hashed DISPATCH
    /// input: a module caches it as a branch selector but MUST NEVER fold it into
    /// any `root()`/`snapshot()` preimage or op encoding. the default is a no-op:
    /// version-invariant modules ignore it; only dual-path modules (forge)
    /// override it. driven ONLY by the agreed boundary version, so every honest
    /// node sets the identical value — never a wall-clock/IO/RNG input.
    fn set_active_version(&mut self, _version: u32) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_required_version_boundary() {
        // max == required passes (the boundary needs exactly what we support).
        assert!(check_required_version(3, 3).is_ok());
        // max > required passes.
        assert!(check_required_version(3, 4).is_ok());
        // max < required fails loud.
        let err = check_required_version(4, 3).expect_err("under-versioned build");
        assert_eq!(err.required_min, 4);
        assert_eq!(err.max_supported, 3);
        assert!(err.to_string().contains("v4"));
        assert!(err.to_string().contains("v3"));
    }

    #[test]
    fn required_min_version_fencepost() {
        let pending = UpgradeCoords {
            name: "v2".into(),
            activation_height: 100,
            to_version: 2,
        };
        // no pending upgrade: always the current version.
        assert_eq!(required_min_version(1, None, 100), 1);
        // below activation: current version.
        assert_eq!(required_min_version(1, Some(&pending), 99), 1);
        // exactly at activation: to_version.
        assert_eq!(required_min_version(1, Some(&pending), 100), 2);
        // past activation: to_version.
        assert_eq!(required_min_version(1, Some(&pending), 250), 2);
    }
}
