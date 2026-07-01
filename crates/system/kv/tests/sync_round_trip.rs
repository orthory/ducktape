//! state-sync round-trip: a fresh `Kv` reconstructs a byte-identical qmdb root by
//! pulling a source store's operation range through commonware's qmdb sync — the
//! smallest proof that layer-3 (module snapshot/install) rebuilds the authenticated
//! root WITHOUT replaying ops in application order.
//!
//! the source OVERWRITES `a` (`1` then `3`), so its committed op log carries a
//! history that a naive "export current key/value pairs and re-apply sorted" could
//! never reproduce — the qmdb root is operation-log ordered, not a canonical merkle
//! over the live key set (see auto-memory reference_qmdb_commonware). only a real
//! sync that ships the ACTUAL proven op range lands on the same root, which is
//! precisely what makes this test discriminating rather than a tautology.

use commonware_runtime::{deterministic, Runner as _, Supervisor as _};
use kv::Kv;
use sdk::{Module, StateRoot};

#[test]
fn synced_store_reconstructs_source_root() {
    deterministic::Runner::default().start(|context| async move {
        // SOURCE: commit several ops, including an overwrite of `a`.
        let mut src = Kv::init(context.child("src"), "src").await;
        src.set(b"a".to_vec(), b"1".to_vec()).await;
        src.set(b"b".to_vec(), b"2".to_vec()).await;
        src.set(b"a".to_vec(), b"3".to_vec()).await; // overwrite: op-log order matters
        let src_root: StateRoot = src.root();
        assert_ne!(src_root, StateRoot::ZERO, "source must have a real root");

        // describe the target (root + op range), THEN hand the source off as the
        // sync resolver (consumes it — order matters).
        let target = src.sync_target().await;
        let resolver = src.into_resolver();

        // JOINER: reconstruct on a FRESH context + namespace by pulling from the
        // resolver. no ops are applied in application order on this side.
        let synced = Kv::sync_from(context.child("dst"), "dst", target, resolver).await;

        // THE PROPERTY: identical qmdb root — the app-hash linkage a joiner needs
        // to be accepted as a consensus participant at the boundary height.
        assert_eq!(
            synced.root(),
            src_root,
            "synced store root must equal the source root"
        );

        // and the live key/value view is correct: `a` overwritten to 3, `b` = 2.
        assert_eq!(synced.get(b"a").await.as_deref(), Some(b"3".as_ref()));
        assert_eq!(synced.get(b"b").await.as_deref(), Some(b"2".as_ref()));
    });
}
