//! proof of the memoized-replay sibling reads: a wasm guest reaches
//! `query-module` / `module-root` through the host `Ctx`, the replay converges,
//! intra-dispatch answers are memoized (one ctx resolution per distinct read),
//! an aborted round's staged writes never leak into the next (no double-apply),
//! and the sibling-read budget rejects deterministically.
//!
//! The fixture is a GENERATED artifact (built from `crates/guests/sibling-wasm`
//! by the module build target); it is committed so this proof is self-contained.

use std::cell::RefCell;

use sdk::{Ctx, Env, Error, Event, Module, Msg, Origin, StateRoot};
use wasm_host::{MAX_SIBLING_READS, WasmModule};

const SIBLING: &[u8] = include_bytes!("fixtures/sibling.component.wasm");

/// a canned sibling world: `directory` answers queries by echoing the request
/// prefixed with `dir:`, `noisy` echoes raw, everything else is unknown. every
/// resolution is counted, so a test can assert exactly how many reads reached
/// the ctx (memo hits never do).
struct MockCtx {
    env: Env,
    queries: RefCell<Vec<(String, Vec<u8>)>>,
    msgs: Vec<Msg>,
    events: Vec<Event>,
}

impl MockCtx {
    fn new(me: &str) -> Self {
        Self {
            env: Env {
                height: 7,
                consensus_time: 0,
                origin: Origin::System,
                me: me.into(),
                cause: sdk::Cause::Direct,
            },
            queries: RefCell::new(Vec::new()),
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
    fn module_root(&self, target: &str) -> Option<StateRoot> {
        (target == "directory").then_some(StateRoot([0xAB; 32]))
    }
    async fn query(&self, target: &str, req: &[u8]) -> Result<Vec<u8>, Error> {
        self.queries.borrow_mut().push((target.into(), req.to_vec()));
        match target {
            "directory" => Ok([b"dir:", req].concat()),
            "noisy" => Ok(req.to_vec()),
            other => Err(Error::UnknownModule(other.into())),
        }
    }
    fn emit_msg(&mut self, msg: Msg) {
        self.msgs.push(msg);
    }
    fn emit_event(&mut self, ev: Event) {
        self.events.push(ev);
    }
}

fn module() -> WasmModule {
    WasmModule::from_bytes("sibling", SIBLING).expect("load")
}

async fn exec(m: &mut WasmModule, ctx: &mut MockCtx, payload: Vec<u8>) -> Result<(), Error> {
    m.execute(
        ctx,
        &Msg {
            target: "sibling".into(),
            payload,
        },
    )
    .await
}

/// read one key out of the canonical snapshot encoding (le-u64 entry count,
/// then sorted le-u64 length-prefixed key/value pairs).
fn state_value(snapshot: &[u8], key: &[u8]) -> Option<Vec<u8>> {
    let take = |buf: &mut &[u8]| -> u64 {
        let (head, rest) = buf.split_first_chunk::<8>().expect("length prefix");
        *buf = rest;
        u64::from_le_bytes(*head)
    };
    let mut buf = snapshot;
    let count = take(&mut buf);
    for _ in 0..count {
        let klen = take(&mut buf) as usize;
        let (k, rest) = buf.split_at(klen);
        buf = rest;
        let vlen = take(&mut buf) as usize;
        let (v, rest) = buf.split_at(vlen);
        buf = rest;
        if k == key {
            return Some(v.to_vec());
        }
    }
    None
}

#[tokio::test]
async fn query_module_resolves_through_ctx_and_memoizes() {
    let mut m = module();
    let mut ctx = MockCtx::new("sibling");

    exec(&mut m, &mut ctx, b"qdirectory:name".to_vec())
        .await
        .expect("execute resolves the sibling read");
    m.commit_block().await.expect("commit");

    // the guest stored the ctx-resolved answer.
    assert_eq!(m.query(b"l").await.expect("stored answer"), b"dir:name");

    // the guest queried twice with the same request; the memo answered the
    // second (and the replay re-treads), so the ctx resolved exactly ONE read.
    assert_eq!(
        ctx.queries.borrow().as_slice(),
        &[("directory".to_string(), b"name".to_vec())],
        "one ctx resolution per distinct read"
    );
}

#[tokio::test]
async fn aborted_replay_rounds_leak_no_staged_writes() {
    let mut m = module();
    let mut ctx = MockCtx::new("sibling");

    // the guest increments its counter BEFORE the sibling query: round 1 stages
    // the increment, pauses on the read, and is discarded; round 2 must start
    // from the pre-dispatch stage or the counter double-applies.
    exec(&mut m, &mut ctx, b"qdirectory:name".to_vec())
        .await
        .expect("execute");
    m.commit_block().await.expect("commit");

    // count == 1 proves the aborted round's stage was rolled back.
    assert_eq!(
        state_value(&m.snapshot(), b"count").as_deref(),
        Some(&1u64.to_le_bytes()[..]),
        "counter must be exactly 1 (no leak from the aborted round)"
    );

    // and it stays exact across further dispatches (2, not 3 or 4).
    exec(&mut m, &mut ctx, b"qdirectory:name".to_vec())
        .await
        .expect("execute again");
    m.commit_block().await.expect("commit");
    let root_direct = {
        let mut fresh = module();
        let mut fctx = MockCtx::new("sibling");
        // same two ops against a fresh instance → identical root: the replay
        // left no hidden state behind.
        exec(&mut fresh, &mut fctx, b"qdirectory:name".to_vec())
            .await
            .expect("fresh 1");
        fresh.commit_block().await.expect("commit");
        exec(&mut fresh, &mut fctx, b"qdirectory:name".to_vec())
            .await
            .expect("fresh 2");
        fresh.commit_block().await.expect("commit");
        fresh.root()
    };
    assert_eq!(m.root(), root_direct, "replay is invisible in the root");
}

#[tokio::test]
async fn module_root_reaches_the_dispatch_snapshot() {
    let mut m = module();
    let mut ctx = MockCtx::new("sibling");

    exec(&mut m, &mut ctx, b"rdirectory".to_vec())
        .await
        .expect("module-root resolves");
    m.commit_block().await.expect("commit");
    assert_eq!(
        state_value(&m.snapshot(), b"root").as_deref(),
        Some(&[0xAB_u8; 32][..]),
        "the sibling's root landed in guest state"
    );

    // an unknown module's root is a real `None` answer, not a pause: the guest
    // sees it and rejects deterministically.
    let err = exec(&mut m, &mut ctx, b"rnowhere".to_vec())
        .await
        .expect_err("unknown module root is None");
    assert!(matches!(err, Error::UnknownModule(id) if id == "nowhere"));
}

#[tokio::test]
async fn sibling_read_budget_is_a_deterministic_rejection() {
    let mut m = module();
    let mut ctx = MockCtx::new("sibling");

    // MAX distinct reads fit the budget…
    let mut ok = b"f".to_vec();
    ok.extend_from_slice(&(MAX_SIBLING_READS as u64).to_le_bytes());
    exec(&mut m, &mut ctx, ok).await.expect("at the budget");

    // …one more is rejected, with the module untouched.
    let root_before = m.root();
    let mut too_many = b"f".to_vec();
    too_many.extend_from_slice(&((MAX_SIBLING_READS + 1) as u64).to_le_bytes());
    let err = exec(&mut m, &mut ctx, too_many)
        .await
        .expect_err("over the budget");
    assert!(matches!(err, Error::Module(m) if m.contains("sibling-read budget")));
    m.abort_block().await.expect("abort");
    assert_eq!(m.root(), root_before, "a rejected op stages nothing");
}

#[tokio::test]
async fn query_with_resolves_and_plain_query_stays_sealed() {
    let m = module();
    let ctx = MockCtx::new("sibling");

    // the ctx-routed read path resolves sibling reads for real.
    let via_ctx = m
        .query_with(&ctx, b"qdirectory:ping")
        .await
        .expect("query_with resolves");
    assert_eq!(via_ctx, b"dir:ping");

    // the ctx-less path answers the sealed stub surface: the guest sees
    // `unsupported` (a deterministic answer, not a pause) and propagates it.
    let err = m.query(b"qdirectory:ping").await.expect_err("sealed");
    assert!(matches!(err, Error::QueryUnsupported));
}
