//! proof of the object-plane host imports (`object-stat` / `object-get` /
//! `object-put`): a wasm guest reaches the content-addressed object store
//! through the host, a just-put id answers same-dispatch stat/get from the
//! staged overlay WITHOUT a pause, an absent id resolves to `None` (through the
//! memoized-replay resolver, not a trap loop), the object-read budget rejects
//! deterministically, and staged puts are discarded on an aborted dispatch.
//!
//! Task 1 wires NO backing (`HostOdb` is implemented by Task 2), so every read
//! that misses the same-dispatch put overlay resolves to `None` — exactly the
//! absent-id contract this proof pins.
//!
//! The fixture is a GENERATED artifact (built from `crates/guests/object-wasm`
//! by the module build target); it is committed so this proof is self-contained.

use sdk::{Ctx, Env, Error, Event, Module, Msg, Origin, StateRoot};
use sha2::{Digest, Sha256};
use wasm_host::{MAX_OBJECT_READS, WasmModule};

const OBJECT: &[u8] = include_bytes!("fixtures/object.component.wasm");

/// the minimal ctx: object reads never route through it (they resolve against
/// the odb backing, `None` in Task 1), so a stub ctx is enough.
struct MockCtx {
    env: Env,
    msgs: Vec<Msg>,
    events: Vec<Event>,
}

impl MockCtx {
    fn new(me: &str) -> Self {
        Self {
            env: Env {
                height: 3,
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
}

fn module() -> WasmModule {
    WasmModule::from_bytes("object", OBJECT).expect("load")
}

async fn exec(m: &mut WasmModule, ctx: &mut MockCtx, payload: Vec<u8>) -> Result<(), Error> {
    m.execute(
        ctx,
        &Msg {
            target: "object".into(),
            payload,
        },
    )
    .await
}

/// the id the host computes for a put: sha256(kind_tag ‖ body).
fn object_id(kind: u8, body: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update([kind]);
    h.update(body);
    h.finalize().into()
}

/// `[b'p', kind, body..]` — put then same-dispatch stat/get of the returned id.
fn put_op(kind: u8, body: &[u8]) -> Vec<u8> {
    let mut p = vec![b'p', kind];
    p.extend_from_slice(body);
    p
}

/// `[b'a', id(32)]` — assert the id is absent (get/stat/has all empty).
fn absent_op(id: &[u8; 32]) -> Vec<u8> {
    let mut p = vec![b'a'];
    p.extend_from_slice(id);
    p
}

/// `[b'b', count_le_u64]` — perform `count` DISTINCT stats (the budget probe).
fn budget_op(count: u64) -> Vec<u8> {
    let mut p = vec![b'b'];
    p.extend_from_slice(&count.to_le_bytes());
    p
}

#[tokio::test]
async fn put_is_visible_to_same_dispatch_stat_and_get() {
    let mut m = module();
    let mut ctx = MockCtx::new("object");

    // the guest puts (kind=2 tree, body "hi"), then stats and gets the returned
    // id IN THE SAME DISPATCH; the op only returns Ok if the overlay answered
    // stat = Some((2, 2)) and get = Some([2, 'h', 'i']) — no pause, no backing.
    exec(&mut m, &mut ctx, put_op(2, b"hi"))
        .await
        .expect("put then same-dispatch stat/get resolve from the staged overlay");

    // and the host computed the content-addressed id the guest stored.
    let stored = m.query(b"i").await.expect("query stored id");
    assert_eq!(stored, object_id(2, b"hi"), "host returns sha256(tag ‖ body)");
}

#[tokio::test]
async fn absent_object_resolves_to_none_not_a_trap_loop() {
    let mut m = module();
    let mut ctx = MockCtx::new("object");

    // an id that was never put: get/stat/has must answer absent through the
    // resolver (one pause + replay), never spin. a trap loop would blow the
    // budget or hang instead of returning Ok.
    let ghost = object_id(1, b"nobody-put-this");
    exec(&mut m, &mut ctx, absent_op(&ghost))
        .await
        .expect("absent id resolves to None");
}

#[tokio::test]
async fn object_read_budget_is_a_deterministic_rejection() {
    let mut m = module();
    let mut ctx = MockCtx::new("object");

    // a modest run of distinct object reads fits the budget (the mechanism —
    // shared with the sibling/store budgets — is proven end-to-end at N=64 by
    // `sibling.rs`; the object budget rides the identical `within_budgets` /
    // `budget_error` path over `object_len`).
    exec(&mut m, &mut ctx, budget_op(16))
        .await
        .expect("under the object-read budget");

    // exceeding MAX_OBJECT_READS distinct reads is a deterministic rejection
    // carrying the object-specific budget message; the module is untouched.
    // NOTE: this is the true-boundary probe (4097 replay rounds, O(N²) — the
    // reason the equally-large store budget is not boundary-tested at all).
    let root_before = m.root();
    let err = exec(&mut m, &mut ctx, budget_op(MAX_OBJECT_READS as u64 + 1))
        .await
        .expect_err("over the object-read budget");
    assert!(matches!(err, Error::Module(m) if m.contains("object-read budget")));
    m.abort_block().await.expect("abort");
    assert_eq!(m.root(), root_before, "a rejected op stages nothing");
}

#[tokio::test]
async fn staged_puts_are_discarded_on_abort() {
    let mut m = module();
    let mut ctx = MockCtx::new("object");

    // a clean put stages the object at the block level…
    exec(&mut m, &mut ctx, put_op(0, b"chunk-bytes"))
        .await
        .expect("put stages the object");
    let id = object_id(0, b"chunk-bytes");

    // …the block aborts, so the staged put is discarded: a later dispatch that
    // probes the id must see it ABSENT (the op only returns Ok on absence).
    m.abort_block().await.expect("abort");
    exec(&mut m, &mut ctx, absent_op(&id))
        .await
        .expect("aborted put left no trace — the id reads absent");
}
