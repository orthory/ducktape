//! the composer over REAL substrates: the repo's fixture components, qmdb
//! stores on a temp runtime, an empty blob plane — genesis, then a reopen of
//! the same stores with a Map snapshot re-installed.

use std::path::PathBuf;

use commonware_runtime::{Runner as _, Supervisor as _};
use noded::bundle::DirCodeSource;
use noded::compose::{Bindings, Boot, BoxFut, Substrates, compose};
use sdk::{MerkleStore, StateRoot, StateSyncHandle};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/host/tests/fixtures")
}

/// one `SnapshotSource` call's future.
type SnapshotFut<'a> = BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>>;

const SELECTION: &[&str] = &["kv", "valset", "acl", "governance", "modules", "runs"];

fn run(body: impl FnOnce(commonware_runtime::tokio::Context, PathBuf) -> BoxFut<'static, ()>) {
    let dir = tempfile::tempdir().unwrap();
    let cfg =
        commonware_runtime::tokio::Config::default().with_storage_directory(dir.path().join("s"));
    let root = dir.path().to_path_buf();
    commonware_runtime::tokio::Runner::new(cfg).start(|context| body(context, root));
}

#[test]
fn composes_wasm_store_map_and_native_over_injected_stores() {
    run(|context, dir| {
        Box::pin(async move {
            let (code, by_id) =
                DirCodeSource::open(&fixtures(), &["acl", "governance", "runs"]).unwrap();
            let validators = vec![vec![7u8; 32]];
            let bindings = Bindings {
                invite: b"t",
                chain_id: "t",
                validators: &validators,
                code_hashes: &by_id,
            };
            let substrates = Substrates {
                forge_repo: dir.join("forge"),
                duckfs_dir: dir.join("duckfs"),
                blobs: blobstore::BlobHandle::default(),
            };
            let mut stores =
                |id: &'static str| -> BoxFut<'_, Result<Box<dyn MerkleStore>, String>> {
                    let context = context.child(id);
                    Box::pin(async move {
                        Ok(
                            Box::new(statesync::qmdb::QmdbStore::init(context, id).await)
                                as Box<dyn MerkleStore>,
                        )
                    })
                };

            // ---- genesis ----
            let modules = compose(
                SELECTION,
                &code,
                &mut stores,
                &substrates,
                &bindings,
                Boot::Genesis,
            )
            .await
            .unwrap();
            let ids: Vec<String> = modules.iter().map(|m| m.id().to_string()).collect();
            assert_eq!(
                ids,
                SELECTION.iter().map(|s| s.to_string()).collect::<Vec<_>>()
            );
            let runs = modules.iter().find(|m| m.id() == "runs").unwrap();
            let Ok(StateSyncHandle::SnapshotBytes(runs_snapshot)) = runs.state_sync_handle() else {
                panic!("a Map tenant syncs by snapshot bytes");
            };
            let genesis = host::Host::genesis(modules).unwrap();
            assert!(
                genesis.module_code_hash("valset").is_none(),
                "native modules carry no code hash"
            );
            assert_eq!(
                genesis.module_code_hash("acl").unwrap(),
                by_id["acl"].to_vec()
            );
            // acl and governance are both store-backed and empty at genesis
            // EXCEPT for governance's seeded `__config` record — the only
            // thing that can set their roots apart.
            assert_ne!(
                genesis.module_root("governance"),
                genesis.module_root("acl"),
                "governance's genesis config was seeded into its store"
            );
            let genesis_root = genesis.root_hash();
            let runs_root = genesis.module_root("runs").unwrap();
            drop(genesis);

            // ---- reopen the same stores: no re-seed, the Map tenant installs
            // its snapshot, and the composed root-hash is the genesis one ----
            let mut snapshots = |id: &'static str| -> SnapshotFut<'_> {
                let bytes = runs_snapshot.clone();
                Box::pin(async move {
                    let is_runs = id == "runs";
                    Ok(is_runs.then_some((bytes, runs_root)))
                })
            };
            let reopened = compose(
                SELECTION,
                &code,
                &mut stores,
                &substrates,
                &bindings,
                Boot::Reopen {
                    snapshots: &mut snapshots,
                },
            )
            .await
            .unwrap();
            let reopened = host::Host::genesis(reopened).unwrap();
            assert_eq!(
                reopened.root_hash(),
                genesis_root,
                "reopen composes the genesis root-hash"
            );
        })
    });
}

#[test]
fn code_hash_drift_and_unknown_ids_are_refused_by_name() {
    run(|context, dir| {
        Box::pin(async move {
            let (code, by_id) = DirCodeSource::open(&fixtures(), &["acl"]).unwrap();
            let (_, with_extra) = DirCodeSource::open(&fixtures(), &["acl", "governance"]).unwrap();
            let bindings = Bindings {
                invite: b"t",
                chain_id: "t",
                validators: &[],
                code_hashes: &by_id,
            };
            let substrates = Substrates {
                forge_repo: dir.join("forge"),
                duckfs_dir: dir.join("duckfs"),
                blobs: blobstore::BlobHandle::default(),
            };
            let mut stores =
                |id: &'static str| -> BoxFut<'_, Result<Box<dyn MerkleStore>, String>> {
                    let context = context.child(id);
                    Box::pin(async move {
                        Ok(
                            Box::new(statesync::qmdb::QmdbStore::init(context, id).await)
                                as Box<dyn MerkleStore>,
                        )
                    })
                };
            let Err(err) = compose(
                &["acl", "governance"],
                &code,
                &mut stores,
                &substrates,
                &bindings,
                Boot::Genesis,
            )
            .await
            else {
                panic!("a wasm module with no genesis code hash composed");
            };
            assert!(
                err.contains("governance"),
                "the refusal names the module: {err}"
            );
            let Err(err) = compose(
                &["not-a-module"],
                &code,
                &mut stores,
                &substrates,
                &bindings,
                Boot::Genesis,
            )
            .await
            else {
                panic!("an id outside the topology composed");
            };
            assert!(
                err.contains("not-a-module"),
                "an unknown id is refused by name: {err}"
            );
            // a stray extra hash would seed the modules registry (and move
            // the genesis root) for a module the selection never composes.
            let extra = Bindings {
                code_hashes: &with_extra,
                ..bindings
            };
            let Err(err) = compose(
                &["acl"],
                &code,
                &mut stores,
                &substrates,
                &extra,
                Boot::Genesis,
            )
            .await
            else {
                panic!("an extra code hash composed");
            };
            assert!(
                err.contains("governance"),
                "the extra key is refused by name: {err}"
            );
        })
    });
}

/// a code source that answers EVERY hash with one fixed component's bytes.
/// a `DirCodeSource` keys itself by what it hashed, so it cannot lie by
/// construction — the composer's re-hash needs a source that can.
struct LiarSource(PathBuf);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for LiarSource {
    async fn fetch(&self, _code_hash: &[u8]) -> Option<Vec<u8>> {
        std::fs::read(&self.0).ok()
    }
}

/// a code source is a lookup, not a guarantee: bytes that do not hash to the
/// genesis entry never seat, or the running code and the modules registry
/// would silently disagree.
#[test]
fn a_code_source_whose_bytes_miss_the_hash_is_refused() {
    run(|context, dir| {
        Box::pin(async move {
            let (_, by_id) = DirCodeSource::open(&fixtures(), &["acl"]).unwrap();
            // the liar answers acl's hash with governance's component.
            let liar = LiarSource(fixtures().join("governance.component.wasm"));
            let bindings = Bindings {
                invite: b"t",
                chain_id: "t",
                validators: &[],
                code_hashes: &by_id,
            };
            let substrates = Substrates {
                forge_repo: dir.join("forge"),
                duckfs_dir: dir.join("duckfs"),
                blobs: blobstore::BlobHandle::default(),
            };
            let mut stores =
                |id: &'static str| -> BoxFut<'_, Result<Box<dyn MerkleStore>, String>> {
                    let context = context.child(id);
                    Box::pin(async move {
                        Ok(
                            Box::new(statesync::qmdb::QmdbStore::init(context, id).await)
                                as Box<dyn MerkleStore>,
                        )
                    })
                };
            let Err(err) = compose(
                &["acl"],
                &liar,
                &mut stores,
                &substrates,
                &bindings,
                Boot::Genesis,
            )
            .await
            else {
                panic!("mismatched code bytes composed");
            };
            assert!(
                err.contains("acl") && err.contains("do not match"),
                "the mismatch is refused by module name: {err}"
            );
        })
    });
}
