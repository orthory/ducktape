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
//! [`StateRoot`] into the global root-hash (see `host::global_root`); how a module
//! *computes* that root — a qmdb merkle root, a git HEAD oid — is private to the
//! module. the host only ever sees `root() -> StateRoot`.
//!
//! this crate also carries the deterministic *system api*: the [`Ctx`] a module
//! touches during state-machine application (own-state r/w lives in `self`;
//! read-only cross-module [`Ctx::query`]/[`Ctx::module_root`]; the deterministic
//! [`Env`]; and intent emission via [`Ctx::emit_msg`]/[`Ctx::emit_event`] — an
//! event is ALSO the lane a host-side worker claims off-consensus work from).
//! the effectful node surface (real network/IO) is a separate layer and out of
//! scope here.
//!
//! keep this crate types + traits with no domain deps (async-trait is the one
//! greenlit exception): everything here is a shared surface for every module.
//! [`codec`] carries the shared zero-dep snapshot-codec primitives on the same
//! everyone-needs-it grounds, and [`genesis_config`] the tiny codec-based
//! GENESIS-CONFIG encoding the host and wasm guests share (per-network
//! parameters installed into a wasm tenant's consensus store at genesis).

pub mod codec;
pub mod genesis_config;
pub mod hash;
pub mod staged_store;
pub mod wire;

pub use staged_store::{StagedStore, store_key};

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
// the deterministic system api — envelopes, env, error, ctx, module seam.
// ============================================================================

/// a write intent at another module (or self). emitted via [`Ctx::emit_msg`] and
/// re-dispatched by the host as a FOLLOW-UP op after the current `execute`
/// returns — never a reentrant mutating call. payload bytes are typed later via
/// per-module crate-root wire types; the host treats them opaquely.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Msg {
    pub target: ModuleId,
    pub payload: Vec<u8>,
}

/// hard cap on an op's declared output ([`Ctx::set_output`]) — matches saga's
/// `MAX_RESULT_BYTES` class; an oversized output is a deterministic rejection
/// of the op, never a truncation.
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;

/// hard cap on a dispatch's assigned stamp ([`Ctx::set_assigned`]) — the
/// module-assigned values of one applied op (a sequence, a revision), carried
/// into the derived-tier op feed. a stamp is a handful of scalars, never a
/// data lane; an oversized stamp is a deterministic rejection of the op.
pub const MAX_ASSIGNED_BYTES: usize = 4 * 1024;

/// a record a module emits via [`Ctx::emit_event`]. it LEAVES the state machine
/// (handed to the effectful node layer) and never re-enters as a follow-up. one
/// lane, two consumer classes: observability readers, and the host-owned worker
/// seam, which try-decodes each event and claims the ones that request
/// off-consensus work (a worker's result returns as an ORDINARY submitted op —
/// the oracle-as-op pattern).
#[derive(Clone, Debug)]
pub struct Event {
    pub source: ModuleId,
    pub payload: Vec<u8>,
}

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

impl Origin {
    /// the cross-module ACTOR STRING convention (inbox source, jobs
    /// submitter/worker, files owner): a module id verbatim, `"ext:"` +
    /// lowercase hex of an external submitter's id bytes, or the literal
    /// `"system"`. the `ext:` prefix is actor DOMAIN SEPARATION — a module
    /// whose id happens to be pure hex can never collide with an external
    /// key's hex. empty external bytes render as `"ext:"`; callers that must
    /// reject an unauthenticated empty submitter check before calling.
    pub fn actor_string(&self) -> String {
        use core::fmt::Write as _;
        match self {
            Origin::Module(id) => id.clone(),
            Origin::External(bytes) => {
                let mut out = String::with_capacity(4 + bytes.len() * 2);
                out.push_str("ext:");
                for b in bytes {
                    let _ = write!(out, "{b:02x}");
                }
                out
            }
            Origin::System => "system".to_owned(),
        }
    }
}

/// reject an empty required string field with the uniform module error
/// message — the op-validation guard shared by tasks/automations.
pub fn require_non_empty(field: &str, value: &str) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::Module(format!("{field} must not be empty")));
    }
    Ok(())
}

/// the field separator inside composite module keys (dispatch keys and saga
/// ids, tagging scope keys): the ASCII unit separator. rejected inside a
/// caller-chosen id by [`validate_id`] so a crafted id can never forge another
/// composite key.
pub const KEY_SEP: char = '\x1f';

/// validate a caller-chosen id: non-empty, within `max_bytes`, and free of the
/// reserved [`KEY_SEP`] — the shared guard for keys that compose with
/// [`KEY_SEP`]. shared by dispatch and tagging. NOT agent's `validate_agent_id`,
/// which is a deliberately separate DNS-label admission rule (an agent id must
/// round-trip as `<id>@agents.duck`), kept distinct so neither rule can
/// silently move the other.
pub fn validate_id(field: &str, value: &str, max_bytes: usize) -> Result<(), Error> {
    if value.is_empty() {
        return Err(Error::Module(format!("{field} must be non-empty")));
    }
    if value.len() > max_bytes {
        return Err(Error::Module(format!(
            "{field} is {} bytes; the cap is {max_bytes}",
            value.len()
        )));
    }
    if value.contains(KEY_SEP) {
        return Err(Error::Module(format!(
            "{field} must not contain the reserved separator"
        )));
    }
    Ok(())
}

/// the "re-derive root, compare, all-or-nothing" guard every in-memory module's
/// `install` shares: a decoded snapshot is adopted ONLY when its recomputed
/// root equals the expected (consensus-committed) root. `actual` is the root
/// the module rehashed from the decoded candidate; a mismatch is a byzantine
/// peer serving bytes that do not hash to the committed state, refused so the
/// caller mutates nothing. this is a state-sync integrity check, never a
/// consensus input — the verdict (accept/reject) is what matters, and it is
/// identical to the per-module guards this replaces.
pub fn verify_snapshot_root(actual: StateRoot, expected: StateRoot) -> Result<(), Error> {
    if actual != expected {
        return Err(Error::Module(format!(
            "snapshot root mismatch: recomputed {actual:?}, expected {expected:?}"
        )));
    }
    Ok(())
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
    /// a module's code is the node binary itself — it has no hot-swappable
    /// component (the default [`Module::swap_code`]).
    SwapUnsupported,
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
            Error::SwapUnsupported => write!(f, "SwapUnsupported"),
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

    /// emit an event — leaves the state machine (observability, and the lane
    /// the host-side worker seam claims off-consensus work from).
    fn emit_event(&mut self, ev: Event);

    /// declare this op's output. staged with the op (rolled back on
    /// rejection); capped at [`MAX_OUTPUT_BYTES`], and exceeding the cap is a
    /// deterministic rejection of the op. last write wins within one dispatch.
    /// the default discards.
    fn set_output(&mut self, _bytes: Vec<u8>) {}

    /// declare this dispatch's assigned stamp — the values the module ASSIGNED
    /// while applying this op (a message sequence, a revision number), which
    /// exist nowhere in the op payload. the host records the stamp on the
    /// dispatch trace, and the derived tier carries it on the op-feed row, so
    /// feed followers (index guests, clients) consume exact assignments
    /// instead of re-deriving them by counting. encoding is module-defined
    /// (the module's own wire codec), opaque to the host like the payload;
    /// capped at [`MAX_ASSIGNED_BYTES`] — exceeding the cap is a deterministic
    /// rejection of the op. last write wins within one dispatch. the default
    /// discards: read-only query ctxs never record a trace.
    fn set_assigned(&mut self, _bytes: Vec<u8>) {}
}

/// the deterministic merkle-KV storage surface a disk-backed module touches —
/// the HOST constructs the concrete store (qmdb today) and INJECTS this handle,
/// so the module is pure logic over it and never names a storage crate. keys
/// are the module's own 32-byte digests (the module owns its logical→digest
/// hashing and its staged overlay); the handle owns durability, the merkle
/// commitment, and the byte-level sync serve surface.
#[async_trait::async_trait(?Send)]
pub trait MerkleStore {
    /// read one hashed key from COMMITTED state.
    async fn get(&self, key: &[u8; ROOT_LEN]) -> Result<Option<Vec<u8>>, Error>;

    /// apply + durably commit ONE batch of hashed-key writes (`None` = delete)
    /// at a block boundary. after this returns, [`MerkleStore::root`] reflects
    /// the batch.
    async fn commit_batch(
        &mut self,
        writes: Vec<([u8; ROOT_LEN], Option<Vec<u8>>)>,
    ) -> Result<(), Error>;

    /// the merkle root over committed state — the module's `root()` verbatim.
    fn root(&self) -> StateRoot;

    /// the committed resolver sync target (root + op-log bounds) behind
    /// [`StateSyncHandle::ResolverBacked`].
    async fn sync_target(&self) -> Result<ResolverSyncTarget, Error>;

    /// serve one byte-level state-sync request against committed state (the
    /// qmdb sync wire; request/response bytes are handle-defined).
    async fn serve_sync(&self, req: &[u8]) -> Result<Vec<u8>, Error>;
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
    /// the global root-hash after a block applies.
    fn root(&self) -> StateRoot;

    /// self-contained committed-state snapshot bytes, for the in-memory
    /// (map-backed) module cohort whose whole state fits one installable blob.
    /// override this — returning `Some(self.snapshot())` — instead of
    /// `state_sync_handle`; the default `state_sync_handle` wraps these bytes in
    /// [`StateSyncHandle::SnapshotBytes`] for you, so a module declares WHAT it
    /// can serve without also knowing the handle enum. `None` (the default) means
    /// no byte snapshot; a resolver-backed (qmdb) module overrides
    /// `state_sync_handle` directly instead.
    fn snapshot_bytes(&self) -> Option<Vec<u8>> {
        None
    }

    /// describe the committed-state sync surface for this module.
    ///
    /// This is called by snapshot orchestration after the host has reached a
    /// block boundary. The default reports [`StateSyncHandle::SnapshotBytes`]
    /// when [`Module::snapshot_bytes`] is `Some` (the in-memory cohort), else
    /// explicit non-coverage; a resolver-backed module overrides this directly
    /// so a live node can advertise exactly what it can serve.
    fn state_sync_handle(&self) -> Result<StateSyncHandle, Error> {
        match self.snapshot_bytes() {
            Some(bytes) => Ok(StateSyncHandle::SnapshotBytes(bytes)),
            None => Ok(StateSyncHandle::Unsupported {
                reason: "module did not declare a state-sync handle".into(),
            }),
        }
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
    /// cursor (the default for modules without an atomic cursor substrate).
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

    /// this module's currently-running CODE identity: the 32-byte content hash
    /// (sha256) of the component bytes it will execute, or `None` for a native
    /// module whose code IS the node binary (nothing to hot-swap). the host
    /// reconciles this against the code registry's committed active hash to
    /// decide whether a boundary swap is needed — a cheap hash compare, so it
    /// re-instantiates a component only on an actual change, never every block.
    /// NEVER a consensus input: code is invisible to `root()` (state, not code,
    /// composes the root-hash), so this is per-node realization bookkeeping only.
    fn code_hash(&self) -> Option<Vec<u8>> {
        None
    }

    /// hot-swap this module's executable CODE in place, KEEPING its host-owned
    /// state — the live-update primitive. the host calls this at a code-registry
    /// activation boundary AFTER it has fetched the out-of-band component bytes
    /// and verified `sha256(bytes)` equals the consensus-committed hash; because
    /// durable state is untouched, `root()` is unchanged and the root-hash stays
    /// continuous across the swap. the default is unsupported — only the wasm
    /// runtime module overrides it (a native module cannot swap its code).
    fn swap_code(&mut self, _component_bytes: &[u8]) -> Result<(), Error> {
        Err(Error::SwapUnsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_actor_string_convention() {
        assert_eq!(Origin::Module("chat".into()).actor_string(), "chat");
        assert_eq!(
            Origin::External(vec![0xAB, 0x01, 0xFF]).actor_string(),
            "ext:ab01ff"
        );
        assert_eq!(Origin::External(Vec::new()).actor_string(), "ext:");
        assert_eq!(Origin::System.actor_string(), "system");
    }

    #[test]
    fn require_non_empty_guard() {
        assert!(require_non_empty("id", "x").is_ok());
        let err = require_non_empty("id", "").unwrap_err().to_string();
        assert!(err.contains("id must not be empty"), "{err}");
    }

    #[test]
    fn validate_id_guard() {
        assert!(validate_id("id", "ok", 128).is_ok());
        assert!(
            validate_id("id", "", 128)
                .unwrap_err()
                .to_string()
                .contains("must be non-empty")
        );
        assert!(
            validate_id("id", "toolong", 3)
                .unwrap_err()
                .to_string()
                .contains("the cap is 3")
        );
        let with_sep = format!("a{KEY_SEP}b");
        assert!(
            validate_id("id", &with_sep, 128)
                .unwrap_err()
                .to_string()
                .contains("reserved separator")
        );
    }

    #[test]
    fn verify_snapshot_root_guard() {
        let a = StateRoot([1u8; ROOT_LEN]);
        let b = StateRoot([2u8; ROOT_LEN]);
        assert!(verify_snapshot_root(a, a).is_ok());
        let err = verify_snapshot_root(a, b).unwrap_err().to_string();
        assert!(err.contains("snapshot root mismatch"), "{err}");
    }

    /// the default `state_sync_handle` reflects `snapshot_bytes`: `Some` ->
    /// `SnapshotBytes`, `None` -> `Unsupported` — the in-memory cohort overrides
    /// only `snapshot_bytes` and the wrapping is shared.
    #[test]
    fn snapshot_bytes_drives_default_handle() {
        struct Snap(Option<Vec<u8>>);
        #[async_trait::async_trait(?Send)]
        impl Module for Snap {
            fn id(&self) -> ModuleId {
                "snap".into()
            }
            fn root(&self) -> StateRoot {
                StateRoot::ZERO
            }
            fn snapshot_bytes(&self) -> Option<Vec<u8>> {
                self.0.clone()
            }
            async fn execute(&mut self, _: &mut dyn Ctx, _: &Msg) -> Result<(), Error> {
                Ok(())
            }
        }
        assert_eq!(
            Snap(Some(vec![1, 2, 3])).state_sync_handle().unwrap(),
            StateSyncHandle::SnapshotBytes(vec![1, 2, 3])
        );
        assert!(matches!(
            Snap(None).state_sync_handle().unwrap(),
            StateSyncHandle::Unsupported { .. }
        ));
    }
}
