//! global-root composition — the app-hash consensus commits to.
//!
//! the global root is a deterministic hash over every module's `(id, root)`.
//! because a module's own [`StateRoot`] already commits to its children (a qmdb
//! merkle root commits to its keys; a git HEAD oid commits to its tree), this
//! one level on top yields the full two-level authentication tree:
//!
//! ```text
//! global_root
//!   ├── ("documents",  qmdb_root) ──▶ commits to all doc blocks
//!   ├── ("forge",      git_head)  ──▶ commits to the whole repo tree
//!   └── ("validators", qmdb_root) ─▶ commits to the validator set
//! ```
//!
//! determinism is the whole job: every validator must produce a byte-identical
//! global root or the chain forks. so modules are sorted by id, and each id is
//! length-prefixed before hashing (otherwise ("ab", r) and ("a", "b"||r) would
//! collide — a classic concatenation ambiguity).

//!
//! ## why a plain hash and not a qmdb-of-heads
//!
//! tempting to make the global head itself a qmdb (module ids -> roots) for
//! "uniform machinery" — DON'T. qmdb's root is an op-log / HISTORY commitment
//! (order-dependent, non-idempotent: re-writing an unchanged head still moves
//! it). an app-hash must be `f(current state)` — order-independent + idempotent —
//! so a node that state-synced from a snapshot computes the same root. this
//! sorted hash already is that; qmdb stays the per-module primitive (`system/kv`),
//! not the global composition. upgrade THIS to a small merkle tree only when a
//! light client needs log-n membership proofs — not before.

use sdk::{Module, ModuleId, StateRoot};
use sha2::{Digest, Sha256};

/// compute the global app-hash over `modules`. order-independent (sorted by id)
/// and unambiguous (length-prefixed ids + a leading module count).
pub fn global_root(modules: &[&dyn Module]) -> StateRoot {
    let mut pairs: Vec<(ModuleId, StateRoot)> =
        modules.iter().map(|m| (m.id(), m.root())).collect();
    pairs.sort_by(|a, b| a.0.cmp(&b.0));

    let mut h = Sha256::new();
    h.update((pairs.len() as u64).to_le_bytes());
    for (id, root) in &pairs {
        h.update((id.len() as u64).to_le_bytes());
        h.update(id.as_bytes());
        h.update(root.0);
    }
    StateRoot(h.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // a stand-in module with a fixed root, so the composition can be tested
    // without standing up a real qmdb/git substrate (that lands in modules/kv).
    struct StubModule {
        id: &'static str,
        root: StateRoot,
    }
    impl Module for StubModule {
        fn id(&self) -> ModuleId {
            self.id.to_string()
        }
        fn root(&self) -> StateRoot {
            self.root
        }
    }

    fn m(id: &'static str, fill: u8) -> StubModule {
        StubModule { id, root: StateRoot([fill; 32]) }
    }

    #[test]
    fn order_independent() {
        let a = m("documents", 1);
        let b = m("forge", 2);
        let c = m("validators", 3);
        let one = global_root(&[&a, &b, &c]);
        let two = global_root(&[&c, &a, &b]);
        assert_eq!(one, two, "global root must not depend on module ordering");
    }

    #[test]
    fn sensitive_to_any_module_root() {
        let a = m("documents", 1);
        let b = m("forge", 2);
        let before = global_root(&[&a, &b]);
        let b2 = m("forge", 9);
        let after = global_root(&[&a, &b2]);
        assert_ne!(before, after, "changing a module root must change the global root");
    }

    #[test]
    fn id_boundary_is_unambiguous() {
        // ("ab", r) vs ("a", r) must not collide — the length-prefix guards the
        // concatenation boundary between id and the next field.
        let x = m("ab", 7);
        let y = m("a", 7);
        assert_ne!(global_root(&[&x]), global_root(&[&y]));
    }

    #[test]
    fn empty_app_has_a_stable_root() {
        assert_eq!(global_root(&[]), global_root(&[]));
    }
}
