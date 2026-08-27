//! the composer over REAL substrates: the repo's fixture components, qmdb
//! stores on a temp runtime, an empty blob plane — genesis, then a reopen of
//! the same stores with a Map snapshot re-installed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use commonware_runtime::{Runner as _, Supervisor as _};
use noded::compose::{Bindings, Boot, BoxFut, Substrates, compose};
use sdk::{MerkleStore, StateRoot, StateSyncHandle};

/// a code source over the fixtures dir, keyed by the sha256 of each component.
struct DirSource(PathBuf, BTreeMap<[u8; 32], &'static str>);

#[async_trait::async_trait(?Send)]
impl host::CodeSource for DirSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        let digest: [u8; 32] = code_hash.try_into().ok()?;
        let id = self.1.get(&digest)?;
        std::fs::read(self.0.join(format!("{id}.component.wasm"))).ok()
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/host/tests/fixtures")
}

type ByHash = BTreeMap<[u8; 32], &'static str>;
/// one `SnapshotSource` call's future.
type SnapshotFut<'a> = BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>>;

fn hashes(ids: &[&'static str]) -> (BTreeMap<String, [u8; 32]>, ByHash) {
    use sha2::Digest as _;
    let mut by_id = BTreeMap::new();
    let mut by_hash = BTreeMap::new();
    for id in ids {
        let bytes = std::fs::read(fixtures().join(format!("{id}.component.wasm"))).unwrap();
        let h: [u8; 32] = sha2::Sha256::digest(&bytes).into();
        by_id.insert(id.to_string(), h);
        by_hash.insert(h, *id);
    }
    (by_id, by_hash)
}

const SELECTION: &[&str] = &["kv", "valset", "acl", "governance", "lifecycle", "runs"];

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
            let (by_id, by_hash) = hashes(&["acl", "governance", "runs"]);
            let code = DirSource(fixtures(), by_hash);
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
fn a_wasm_module_without_a_code_hash_is_refused_by_id() {
    run(|context, dir| {
        Box::pin(async move {
            let (by_id, by_hash) = hashes(&["acl"]);
            let code = DirSource(fixtures(), by_hash);
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
        })
    });
}
