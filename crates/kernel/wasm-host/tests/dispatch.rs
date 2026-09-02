//! L2/L3 proof: a wasm module dispatches through the `sdk::Module` seam exactly
//! like a native one — staged writes, block-boundary commit, deterministic root,
//! and a hot-swap that keeps the host-owned state.
//!
//! The fixture is a GENERATED artifact (built from `crates/guests/hello-wasm`
//! by the module build target); it is committed so this proof is self-contained.

use sdk::{Env, Module, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

const HELLO: &[u8] = include_bytes!("fixtures/hello.component.wasm");

use sdk_testkit::TestCtx;

fn mock(me: &str) -> TestCtx {
    TestCtx::with_env(Env {
        height: 1,
        consensus_time: 0,
        origin: Origin::System,
        me: me.into(),
    })
}

async fn inc(m: &mut WasmModule, ctx: &mut TestCtx) {
    m.execute(
        ctx,
        &Msg {
            target: "hello".into(),
            payload: b"inc".to_vec(),
        },
    )
    .await
    .expect("inc executes");
}

fn count(bytes: Vec<u8>) -> u64 {
    u64::from_le_bytes(bytes.try_into().expect("8-byte count"))
}

#[tokio::test]
async fn dispatch_commit_query_roundtrip() {
    let mut m = WasmModule::from_bytes("hello", HELLO).expect("load");
    let mut ctx = mock("hello");
    let root0 = m.root();

    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;

    // staged, not committed → root still reflects committed (empty) state.
    assert_eq!(m.root(), root0, "root reflects committed state only");

    m.commit_block().await.expect("commit");
    assert_ne!(m.root(), root0, "commit advances the root");

    assert_eq!(count(m.query(b"").await.expect("query")), 3);
    assert_eq!(ctx.events().len(), 3, "one event per inc, drained to ctx");
    assert_eq!(ctx.events()[2].source, "hello");
}

#[tokio::test]
async fn abort_discards_staged() {
    let mut m = WasmModule::from_bytes("hello", HELLO).expect("load");
    let mut ctx = mock("hello");
    let root0 = m.root();
    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;
    m.abort_block().await.expect("abort");
    assert_eq!(m.root(), root0, "abort leaves no trace");
    assert_eq!(count(m.query(b"").await.expect("query")), 0);
}

#[tokio::test]
async fn deterministic_root_across_instances() {
    async fn run() -> StateRoot {
        let mut m = WasmModule::from_bytes("hello", HELLO).expect("load");
        let mut ctx = mock("hello");
        for _ in 0..5 {
            inc(&mut m, &mut ctx).await;
        }
        m.commit_block().await.expect("commit");
        m.root()
    }
    assert_eq!(
        run().await,
        run().await,
        "same ops → identical root on independent instances"
    );
}

#[tokio::test]
async fn snapshot_install_round_trip() {
    let mut src = WasmModule::from_bytes("hello", HELLO).expect("load");
    let mut ctx = mock("hello");
    inc(&mut src, &mut ctx).await;
    inc(&mut src, &mut ctx).await;
    src.commit_block().await.expect("commit");
    let root = src.root();

    // sha256(snapshot()) IS the root — the checkpoint ships the exact preimage.
    let bytes = src.snapshot();
    let mut dst = WasmModule::from_bytes("hello", HELLO).expect("load");
    dst.install(&bytes, root).expect("verify-then-adopt");
    assert_eq!(dst.root(), root, "installed root equals the source root");
    assert_eq!(count(dst.query(b"").await.expect("query")), 2);

    // tampered / truncated / trailing bytes are refused, target untouched.
    let mut flipped = bytes.clone();
    let mid = flipped.len() / 2;
    flipped[mid] ^= 0x01;
    assert!(dst.install(&flipped, root).is_err());
    assert!(dst.install(&bytes[..bytes.len() - 1], root).is_err());
    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(dst.install(&trailing, root).is_err());
    assert_eq!(dst.root(), root, "failed installs left the target untouched");
}

#[tokio::test]
async fn hot_swap_keeps_state() {
    let mut m = WasmModule::from_bytes("hello", HELLO).expect("load");
    let mut ctx = mock("hello");
    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;
    m.commit_block().await.expect("commit");
    let before = m.query(b"").await.expect("query");
    assert_eq!(count(before.clone()), 2);

    // swap to the SAME code (stand-in for a new version); state must survive.
    m.swap_code(HELLO).expect("swap");
    assert_eq!(m.query(b"").await.expect("query"), before, "swap keeps state");

    // the swapped code still executes against the kept store.
    inc(&mut m, &mut ctx).await;
    m.commit_block().await.expect("commit");
    assert_eq!(count(m.query(b"").await.expect("query")), 3);
}

/// THE READINESS PROBE MUST ACTUALLY LOAD. A validator signals `SwapReady` off
/// this answer, and byte residency alone let a node on an older binary arm a
/// swap it then rejected every op to (#1297) — so the check has to run the real
/// compile + instantiate, not merely look at the bytes.
#[test]
fn check_loadable_accepts_a_real_component_and_refuses_garbage() {
    WasmModule::check_loadable(HELLO).expect("the shipped fixture loads on this binary");

    for (what, bytes) in [
        ("empty", Vec::new()),
        ("not wasm at all", b"this is not a component".to_vec()),
        // a valid wasm preamble with a truncated body: passes the magic
        // number, dies in validation — the shape a half-fetched blob has.
        ("truncated", b"\0asm\x0d\0\x01\0\x01\x02".to_vec()),
    ] {
        assert!(
            WasmModule::check_loadable(&bytes).is_err(),
            "{what} bytes must never be reported loadable"
        );
    }
}
