//! the module interface crate — the ONLY crate a feature module may depend on.
//!
//! a super-app feature (documents, forge, chat, tasks, …) is an isolated module:
//! a crate that implements [`Module`] and depends on `sdk` and nothing else in
//! the workspace. the host composes each module's [`StateRoot`] into the global
//! app-hash (see the `state` crate); how a module *computes* that root — a qmdb
//! merkle root, a git HEAD oid — is private to the module. the host only ever
//! sees `root() -> StateRoot`.
//!
//! keep this crate types + traits with no domain deps: everything here is a
//! shared surface for every module.

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

/// a module's stable identity within the app. assigned at genesis and part of
/// consensus state — NOT per-node config — so every validator composes the same
/// global root in the same order.
pub type ModuleId = String;

/// the host-facing surface of a feature module. the dispatch/apply seam joins
/// this trait in the next slice; for the authenticated-state backbone a module
/// only has to name itself and expose its root.
pub trait Module {
    /// this module's genesis-assigned id (e.g. "documents", "forge").
    fn id(&self) -> ModuleId;

    /// the module's current authenticated root. called by the host to fold into
    /// the global app-hash after a block applies.
    fn root(&self) -> StateRoot;
}
