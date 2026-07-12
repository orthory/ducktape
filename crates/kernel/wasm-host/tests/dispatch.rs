//! L2/L3 proof: a wasm module dispatches through the `sdk::Module` seam exactly
//! like a native one — staged writes, block-boundary commit, deterministic root,
//! and a hot-swap that keeps the host-owned state.
//!
//! The fixture is a GENERATED artifact (built from `crates/examples/hello-wasm`
//! by the module build target); it is committed so this proof is self-contained.

use sdk::{Ctx, Effect, Env, Error, Event, Module, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

const HELLO: &[u8] = include_bytes!("fixtures/hello.component.wasm");

struct MockCtx {
    env: Env,
    msgs: Vec<Msg>,
    events: Vec<Event>,
}

impl MockCtx {
    fn new(me: &str) -> Self {
        Self {
            env: Env {
                height: 1,
                consensus_time: 0,
                origin: Origin::System,
                me: me.into(),
                protocol_version: 0,
            },
            msgs: Vec::new(),
            events: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Ctx for MockCtx {
    fn env(&self) -> &Env {
        &self.env
    }
    fn module_root(&self, _target: &str) -> Option<StateRoot> {
        None
    }
    async fn query(&self, _target: &str, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Err(Error::QueryUnsupported)
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.msgs.push(msg);
    }
    fn emit_event(&mut self, ev: Event) {
        self.events.push(ev);
    }
    fn request_effect(&mut self, _eff: Effect) {}
}

async fn inc(m: &mut WasmModule, ctx: &mut MockCtx) {
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
    let mut ctx = MockCtx::new("hello");
    let root0 = m.root();

    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;
    inc(&mut m, &mut ctx).await;

    // staged, not committed → root still reflects committed (empty) state.
    assert_eq!(m.root(), root0, "root reflects committed state only");

    m.commit_block().await.expect("commit");
    assert_ne!(m.root(), root0, "commit advances the root");

    assert_eq!(count(m.query(b"").await.expect("query")), 3);
    assert_eq!(ctx.events.len(), 3, "one event per inc, drained to ctx");
    assert_eq!(ctx.events[2].source, "hello");
}

#[tokio::test]
async fn abort_discards_staged() {
    let mut m = WasmModule::from_bytes("hello", HELLO).expect("load");
    let mut ctx = MockCtx::new("hello");
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
        let mut ctx = MockCtx::new("hello");
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
    let mut ctx = MockCtx::new("hello");
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
    let mut ctx = MockCtx::new("hello");
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
