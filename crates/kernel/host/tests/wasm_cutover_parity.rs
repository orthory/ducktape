//! the cutover-continuity proof for the first real wasm tenant: the
//! `directory-wasm` component and the native `Directory` are BYTE-COMPATIBLE.
//! the same op sequence yields the same root() after every block, the same
//! snapshot bytes, and the same query replies — and each side's snapshot
//! installs into the other against the same root. a native→wasm cutover of
//! this module therefore preserves the app-hash and restores pre-cutover
//! workspaces unchanged.

use directory::{
    DirMsg, DirQuery, DirReply, Directory, decode_reply, encode_msg, encode_query,
};
use host::Host;
use sdk::{Module, Msg, StateSyncHandle};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/directory-wasm` by the
/// module build target; committed so this proof is self-contained.
const DIRECTORY_WASM: &[u8] = include_bytes!("fixtures/directory.component.wasm");

fn wasm_directory() -> WasmModule {
    WasmModule::from_bytes("directory", DIRECTORY_WASM).expect("load component")
}

fn set(key: &str, value: &str) -> Msg {
    Msg {
        target: "directory".into(),
        payload: encode_msg(&DirMsg::Set {
            key: key.into(),
            value: value.into(),
        }),
    }
}

fn get(key: &str) -> Vec<u8> {
    encode_query(&DirQuery::Get { key: key.into() })
}

fn snapshot_bytes(m: &dyn Module) -> Vec<u8> {
    match m.state_sync_handle().expect("handle") {
        StateSyncHandle::SnapshotBytes(b) => b,
        other => panic!("expected snapshot bytes, got {other:?}"),
    }
}

#[test]
fn same_ops_same_root_same_snapshot_same_replies() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let mut native = Host::genesis(vec![Box::new(Directory::new("directory"))]).expect("genesis");
    let mut wasm = Host::genesis(vec![Box::new(wasm_directory())]).expect("genesis");

    // identical genesis roots (both commit to the empty map the same way).
    assert_eq!(
        native.module_root("directory"),
        wasm.module_root("directory"),
        "genesis roots diverge"
    );

    // the same op sequence, block by block — roots stay identical at every
    // boundary (this is the app-hash-continuity claim, module-locally).
    let ops = [
        set("a", "1"),
        set("name", "world"),
        set("a", "overwritten"),
        set("Ω", "unicode-value"),
    ];
    for op in ops {
        native.submit(op.clone()).await.expect("native submit");
        wasm.submit(op).await.expect("wasm submit");
        assert_eq!(
            native.module_root("directory"),
            wasm.module_root("directory"),
            "roots diverge after a block"
        );
    }

    // identical replies, including the None shape.
    for key in ["a", "name", "Ω", "absent"] {
        let n = native.query("directory", &get(key)).await.expect("native");
        let w = wasm.query("directory", &get(key)).await.expect("wasm");
        assert_eq!(n, w, "replies diverge for {key:?}");
    }
    let reply = wasm.query("directory", &get("a")).await.expect("reply");
    assert_eq!(
        decode_reply(&reply).unwrap(),
        DirReply::Value(Some("overwritten".into()))
    );
}

#[test]
fn snapshots_cross_install_between_runtimes() {
    futures::executor::block_on(cross_install_inner());
}

async fn cross_install_inner() {
    // build committed state on a NATIVE module (module-level, no host).
    let mut native = Directory::new("directory");
    native.set("k1".into(), "v1".into());
    native.set("k2".into(), "v2".into());
    let native_root = native.root();
    let native_snapshot = native.snapshot();

    // a pre-cutover (native) snapshot installs into the wasm module unchanged —
    // the restore path a cutover node takes over an existing workspace.
    let mut wasm = wasm_directory();
    wasm.install(&native_snapshot, native_root)
        .expect("native snapshot installs into the wasm runtime");
    assert_eq!(wasm.root(), native_root, "installed root must match");
    let reply = wasm.query(&get("k2")).await.expect("query");
    assert_eq!(
        decode_reply(&reply).unwrap(),
        DirReply::Value(Some("v2".into()))
    );

    // and the wasm module's snapshot is byte-identical, so it round-trips back
    // into a fresh NATIVE module (the rollback path).
    assert_eq!(
        snapshot_bytes(&wasm),
        native_snapshot,
        "snapshot encodings diverge"
    );
    let mut back = Directory::new("directory");
    back.install(&snapshot_bytes(&wasm), native_root)
        .expect("wasm snapshot installs into the native module");
    assert_eq!(back.root(), native_root);
}
