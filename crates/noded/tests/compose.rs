//! the composer over REAL substrates: the repo's fixture components, qmdb
//! stores on a temp runtime, an empty blob plane — genesis, then a reopen of
//! the same stores off the modules registry with a map snapshot re-installed;
//! and the ONE wasm path's refusals, by name.

use std::path::PathBuf;
use std::time::Duration;

use commonware_runtime::Runner as _;
use host::CapturePayloads;
use noded::bundle::{DirCodeSource, qmdb_stores};
use noded::compose::{
    Admissions, Bindings, Boot, BoxFut, Substrates, check_realizable, compose,
};
use sdk::{StateRoot, StateSyncHandle};
use wasm_host::{Backing, Shape};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../kernel/host/tests/fixtures")
}

/// one `SnapshotSource` call's future.
type SnapshotFut<'a> = BoxFut<'a, Result<Option<(Vec<u8>, StateRoot)>, String>>;

const SELECTION: &[&str] = &["kv", "valset", "acl", "governance", "modules", "runs"];

const BINDINGS: Bindings<'static> = Bindings {
    invite: b"t",
    chain_id: "t",
};

fn run(body: impl FnOnce(commonware_runtime::tokio::Context, PathBuf) -> BoxFut<'static, ()>) {
    let dir = tempfile::tempdir().unwrap();
    let cfg =
        commonware_runtime::tokio::Config::default().with_storage_directory(dir.path().join("s"));
    let root = dir.path().to_path_buf();
    commonware_runtime::tokio::Runner::new(cfg).start(|context| body(context, root));
}

fn substrates(dir: &std::path::Path) -> Substrates {
    Substrates {
        forge_repo: dir.join("forge"),
        duckfs_dir: dir.join("duckfs"),
        blobs: blobstore::BlobHandle::default(),
    }
}

#[test]
fn composes_wasm_store_map_and_native_over_injected_stores() {
    run(|context, dir| {
        Box::pin(async move {
            let (code, by_id) =
                DirCodeSource::open(&fixtures(), &["acl", "governance", "runs"]).unwrap();
            let validators = vec![vec![7u8; 32]];
            let substrates = substrates(&dir);
            let mut stores = qmdb_stores(&context);

            // ---- genesis ----
            let genesis = compose(
                SELECTION,
                &code,
                &mut stores,
                &substrates,
                &BINDINGS,
                Boot::Genesis {
                    validators: &validators,
                    bundle: &by_id,
                },
            )
            .await
            .unwrap();
            let mut ids: Vec<String> = genesis
                .module_roots()
                .into_iter()
                .map(|(id, _)| id)
                .collect();
            ids.sort_unstable();
            let mut want: Vec<String> = SELECTION.iter().map(|s| s.to_string()).collect();
            want.sort_unstable();
            assert_eq!(ids, want);
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
            let (captured, _) =
                genesis.capture_current_snapshot(0, CapturePayloads::All, || Duration::ZERO);
            let runs = captured.module("runs").expect("runs composed");
            let StateSyncHandle::SnapshotBytes(runs_snapshot) = runs.state_sync.clone() else {
                panic!("a map tenant syncs by snapshot bytes");
            };
            let runs_root = runs.root;
            drop(genesis);

            // ---- reopen the same stores at block zero: the wasm set comes off
            // the modules registry, no store re-seeds, the map tenant installs
            // its snapshot, and the composed root-hash is the genesis one ----
            let mut snapshots = |id: &str, backing: Backing| -> SnapshotFut<'_> {
                assert_ne!(backing, Backing::Store, "a store-backed module is never asked");
                let bytes = runs_snapshot.clone();
                let is_runs = id == "runs";
                Box::pin(async move { Ok(is_runs.then_some((bytes, runs_root))) })
            };
            let reopened = compose(
                SELECTION,
                &code,
                &mut stores,
                &substrates,
                &BINDINGS,
                Boot::Reopen {
                    height: 0,
                    snapshots: &mut snapshots,
                },
            )
            .await
            .unwrap();
            assert_eq!(
                reopened.root_hash(),
                genesis_root,
                "reopen composes the genesis root-hash"
            );
        })
    });
}

#[test]
fn bundle_drift_and_unknown_ids_are_refused_by_name() {
    run(|context, dir| {
        Box::pin(async move {
            let (code, by_id) = DirCodeSource::open(&fixtures(), &["acl"]).unwrap();
            let (_, with_extra) = DirCodeSource::open(&fixtures(), &["acl", "governance"]).unwrap();
            let substrates = substrates(&dir);
            let mut stores = qmdb_stores(&context);
            let Err(err) = compose(
                &["acl", "governance"],
                &code,
                &mut stores,
                &substrates,
                &BINDINGS,
                Boot::Genesis {
                    validators: &[],
                    bundle: &by_id,
                },
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
                &BINDINGS,
                Boot::Genesis {
                    validators: &[],
                    bundle: &by_id,
                },
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
            let Err(err) = compose(
                &["acl"],
                &code,
                &mut stores,
                &substrates,
                &BINDINGS,
                Boot::Genesis {
                    validators: &[],
                    bundle: &with_extra,
                },
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

    fn origin(&self) -> &'static str {
        "test_liar"
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
            let substrates = substrates(&dir);
            let mut stores = qmdb_stores(&context);
            let Err(err) = compose(
                &["acl"],
                &liar,
                &mut stores,
                &substrates,
                &BINDINGS,
                Boot::Genesis {
                    validators: &[],
                    bundle: &by_id,
                },
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

/// a shape this host cannot realize is refused BY NAME, before any substrate
/// is touched: an odb declaration under an id the host has no substrate for,
/// or a config key no network binds. the same check the readiness probe runs
/// before a validator signals a swap ready.
#[test]
fn a_shape_the_host_cannot_realize_is_refused_by_name() {
    let odb = Shape {
        backing: Backing::Odb,
        config: Vec::new(),
        committed_queries: false,
    };
    let err = check_realizable("kanban", &odb).unwrap_err();
    assert!(
        err.contains("kanban") && err.contains("odb"),
        "an odb declaration without a substrate names the module: {err}"
    );
    check_realizable("files", &odb).expect("files has an odb substrate");
    check_realizable("forge", &odb).expect("forge has an odb substrate");

    let unknown_key = Shape {
        backing: Backing::Store,
        config: vec!["tenant".into()],
        committed_queries: false,
    };
    let err = check_realizable("kanban", &unknown_key).unwrap_err();
    assert!(
        err.contains("kanban") && err.contains("tenant"),
        "an unbound config key names the module and the key: {err}"
    );
    let bound = Shape {
        backing: Backing::Store,
        config: vec![
            sdk::genesis_config::INVITE.into(),
            sdk::genesis_config::CHAIN_ID.into(),
        ],
        committed_queries: true,
    };
    check_realizable("kanban", &bound).expect("the network binds both keys");
}

/// a post-genesis admission builds through the SAME wasm path a genesis
/// tenant took: the factory wraps the bytes over the substrate they declare
/// (a map-declared fixture over a fresh map) and refuses a declaration the
/// host cannot realize under that id (the odb-declared fixture under an id
/// with no substrate), by name.
#[test]
fn admissions_build_through_the_one_wasm_path() {
    run(|context, dir| {
        Box::pin(async move {
            let hello = std::fs::read(fixtures().join("hello.component.wasm")).unwrap();
            let object = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../kernel/wasm-host/tests/fixtures/object.component.wasm"),
            )
            .unwrap();
            let substrates = substrates(&dir);
            let admissions = Admissions::new(&context, &substrates, &BINDINGS);
            let admitted = host::ModuleFactory::instantiate(&admissions, "hello", &hello)
                .await
                .expect("a map-declared component admits over a fresh map");
            let host::Admitted::Module(module) = admitted else {
                panic!("the hello fixture is a `ducktape:module`");
            };
            assert_eq!(module.id(), "hello");
            let err = host::ModuleFactory::instantiate(&admissions, "kanban", &object)
                .await
                .err()
                .expect("an odb declaration under an id with no substrate is refused");
            assert!(
                err.to_string().contains("kanban"),
                "the refusal names the module: {err}"
            );
            // and the ONE refusal that is not fail-closed: bytes that are no
            // `ducktape:module` at all are another plane's commitment record.
            let netstack = std::fs::read(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../networking/netstack-machine/component.wasm"),
            )
            .unwrap();
            let admitted = host::ModuleFactory::instantiate(&admissions, "netstack", &netstack)
                .await
                .expect("a foreign-world component is answered, not errored");
            assert!(
                matches!(admitted, host::Admitted::ForeignAbi),
                "the netstack guest is no module admission"
            );
        })
    });
}
