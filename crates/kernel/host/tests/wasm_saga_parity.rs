//! the adapter-port equivalence proof for the saga cutover: the `saga-wasm`
//! component (the NATIVE `saga` crate compiled to wasm behind `guest-adapter`)
//! and the native `SagaModule` answer the SAME op sequence with IDENTICAL
//! query replies, emit IDENTICAL work-order events, and their roots move in
//! lockstep. the state-schema break is pinned with saga's OWN genesis shape:
//! unlike the ZERO-sentinel modules, saga's empty canonical encoding (a bare
//! zero count) hashes to the SAME digest as the wasm port's empty host-KV
//! store, so the roots COINCIDE at genesis and diverge at the first committed
//! write — never to re-converge (revision 2 declares the break regardless).
//!
//! saga is the deterministic half of the async engine, so this proof leans on
//! three surfaces beyond the usual reply/root matrix:
//!
//! * WORKER EVENTS: trigger / retry / accept emits a [`WorkerRequest`], while
//!   terminal transitions may emit a cancellation control. the host-side
//!   worker seam decodes both out of `BlockOutcome::events`. both runtimes must
//!   surface the byte-identical event stream, or the reactor would feed workers
//!   differently across the cutover.
//! * P6 CALLBACKS: every terminal transition with a `reply_to` emits a
//!   same-block [`SagaCallback`] msg. a native recorder module ("req") folds
//!   every callback it receives into its root on BOTH hosts, so a missing,
//!   duplicated, or reordered callback diverges the recorder roots.
//! * SIBLING-READ ASSIGNMENT: the production constructor
//!   (`SagaModule::with_assignment("saga", "valset", "capability",
//!   LeasePolicy::Strict)` — `bin/node/src/host_state.rs`) resolves every
//!   attempt's assignee through valset/capability queries; on the wasm side
//!   those are host-routed `query-module` reads resolved through the
//!   runtime's memoized replay against the REAL native siblings both hosts
//!   carry.

use host::{BlockContext, BlockOutcome, Host, MemberOutcome, SubmitError};
use capability::{CapabilityMsg, CapabilityRegistry, encode_msg as capability_encode_msg};
use saga::{
    LeasePolicy, SagaModule, SagaMsg, SagaQuery, SagaReply, SagaStatus, WorkerRequest,
    decode_reply, decode_worker_control, decode_worker_request, encode_msg, encode_query,
    MAX_CAPABILITY_BYTES, MAX_ERROR_BYTES, MAX_REPLY_PAYLOAD_BYTES, MAX_RESULT_BYTES,
};
use sdk::{Ctx, Error, Module, ModuleId, Msg, Origin, StateRoot};
use std::collections::BTreeMap;
use valset::Valset;
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/saga-wasm` by the module
/// build target; committed so this proof is self-contained.
const SAGA_WASM: &[u8] = include_bytes!("fixtures/saga.component.wasm");

fn wasm_saga() -> WasmModule {
    WasmModule::from_bytes("saga", SAGA_WASM)
        .expect("load component")
        // the adapter port's host-KV snapshot is revision 2 of the saga
        // canonical state.
        .with_state_schema_revision(2)
}

/// the production wiring, verbatim (`bin/node/src/host_state.rs`).
fn native_saga() -> SagaModule {
    SagaModule::with_assignment("saga", "valset", "capability", LeasePolicy::Strict)
}

/// a 32-byte member key. the ordered lane hands modules verified ed25519 ids;
/// the parity claim only needs them distinct, non-empty, and identically
/// byte-compared by valset membership, capability announcements, and saga's
/// strict lease gate — no signatures cross this proof.
fn key(tag: u8) -> Vec<u8> {
    vec![tag; 32]
}

fn seeded_valset(members: &[Vec<u8>]) -> Valset {
    let mut valset = Valset::new("valset");
    for m in members {
        valset.insert(m.clone());
    }
    valset
}

/// the requester recorder: a native module both hosts carry under the id the
/// triggers name as `reply_to`. it commits to the byte-concatenation of every
/// saga callback it receives, so a callback that diverged (or went missing)
/// between the runtimes diverges the recorder roots. staged/committed split
/// keeps the block boundary honest (an aborted block leaves no trace here
/// either).
struct Recorder {
    staged: Vec<Vec<u8>>,
    committed: Vec<u8>,
}

impl Recorder {
    fn new() -> Self {
        Self {
            staged: Vec::new(),
            committed: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Recorder {
    fn id(&self) -> ModuleId {
        "req".into()
    }
    fn root(&self) -> StateRoot {
        StateRoot(sha2::Sha256::digest(&self.committed).into())
    }
    async fn execute(&mut self, _ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged.push(msg.payload.clone());
        Ok(())
    }
    async fn commit_block(&mut self) -> Result<(), Error> {
        for payload in self.staged.drain(..) {
            self.committed.extend_from_slice(&payload);
        }
        Ok(())
    }
    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

use sha2::Digest as _;

fn native_host(members: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(native_saga()),
        Box::new(seeded_valset(members)),
        Box::new(CapabilityRegistry::new("capability", Some("valset".into()))),
        Box::new(Recorder::new()),
    ])
    .expect("genesis")
}

fn wasm_host_(members: &[Vec<u8>]) -> Host {
    Host::genesis(vec![
        Box::new(wasm_saga()),
        Box::new(seeded_valset(members)),
        Box::new(CapabilityRegistry::new("capability", Some("valset".into()))),
        Box::new(Recorder::new()),
    ])
    .expect("genesis")
}

/// one block's agreed context. consensus_time == height, as on the real
/// validator network — saga compares lease/deadline views against
/// consensus_time, so keeping them equal keeps the sweep arithmetic honest.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: height,
        origin,
    }
}

fn saga_op(m: &SagaMsg) -> Msg {
    Msg {
        target: "saga".into(),
        payload: encode_msg(m),
    }
}

/// trigger parameters as a STRUCT (enum variants take no functional-record
/// updates), so call sites override exactly the fields a scenario is about.
struct Trig {
    saga_id: String,
    spec: Vec<u8>,
    reply_to: Option<String>,
    reply_payload: Vec<u8>,
    deadline: Option<u64>,
    max_attempts: u32,
    lease_views: Option<u64>,
    capability: Option<String>,
    demands: BTreeMap<String, u64>,
    pinned_assignee: Option<Vec<u8>>,
}

impl From<Trig> for SagaMsg {
    fn from(t: Trig) -> Self {
        SagaMsg::Trigger {
            saga_id: t.saga_id,
            spec: t.spec,
            reply_to: t.reply_to,
            reply_payload: t.reply_payload,
            deadline: t.deadline,
            max_attempts: t.max_attempts,
            lease_views: t.lease_views,
            capability: t.capability,
            demands: t.demands,
            pinned_assignee: t.pinned_assignee,
        }
    }
}

/// a trigger with fire-and-forget defaults; call sites override fields inline.
fn trigger(id: &str) -> Trig {
    Trig {
        saga_id: id.into(),
        spec: format!("work:{id}").into_bytes(),
        reply_to: Some("req".into()),
        reply_payload: format!("corr:{id}").into_bytes(),
        deadline: None,
        max_attempts: 1,
        lease_views: None,
        capability: None,
        demands: BTreeMap::new(),
        pinned_assignee: None,
    }
}

fn oracle(id: &str, attempt: u32, outcome: Result<Vec<u8>, String>) -> Msg {
    saga_op(&SagaMsg::OracleResult {
        saga_id: id.into(),
        attempt,
        outcome,
        usage: None,
    })
}

fn announce(capabilities: &[&str], resources: &[(&str, u64)]) -> Msg {
    Msg {
        target: "capability".into(),
        payload: capability_encode_msg(&CapabilityMsg::Announce {
            capabilities: capabilities.iter().map(|s| s.to_string()).collect(),
            resources: resources
                .iter()
                .map(|(k, v)| (k.to_string(), *v))
                .collect(),
        }),
    }
}

/// the read matrix: per-id Get (present and absent ids alike), the crank
/// pump's NextExpiry, and both members' AssignedPending projections (the
/// resident worker pump's read — reconstructed WorkerRequests must agree).
async fn replies(h: &Host, ids: &[&str], members: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut queries = vec![encode_query(&SagaQuery::NextExpiry)];
    for id in ids {
        queries.push(encode_query(&SagaQuery::Get {
            saga_id: (*id).into(),
        }));
    }
    queries.push(encode_query(&SagaQuery::Get {
        saga_id: "absent".into(),
    }));
    for m in members {
        queries.push(encode_query(&SagaQuery::AssignedPending {
            assignee: m.to_vec(),
        }));
    }
    let mut out = Vec::new();
    for q in &queries {
        out.push(h.query("saga", q).await.expect("query"));
    }
    out
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("saga").expect("saga registered")
}

const SIBLING_IDS: [&str; 3] = ["valset", "capability", "req"];

/// the block's event trace as comparable tuples, and the saga work orders
/// decoded from it — the exact surface the host-side worker seam consumes.
fn event_tuples(out: &BlockOutcome) -> Vec<(String, Vec<u8>)> {
    out.events
        .iter()
        .map(|e| (e.source.clone(), e.payload.clone()))
        .collect()
}

fn worker_requests(out: &BlockOutcome) -> Vec<WorkerRequest> {
    out.events
        .iter()
        .filter(|e| e.source == "saga")
        .filter_map(|e| match decode_worker_request(&e.payload) {
            Ok(request) => Some(request),
            Err(_) => {
                decode_worker_control(&e.payload)
                    .expect("saga events are worker requests or controls");
                None
            }
        })
        .collect()
}

/// submit one ACCEPTED op to both hosts and assert the full parity claim:
/// identical replies, identical event traces (and decoded work orders),
/// per-sibling cross-host agreement (the recorder root carries the callback
/// lane), and lockstep saga-root movement.
/// (one argument per invariant knob; a builder would only obscure the matrix.)
#[allow(clippy::too_many_arguments)]
async fn roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    ids: &[&str],
    members: &[&[u8]],
    height: u64,
    origin: Origin,
    m: Msg,
    moves: bool,
) -> (Vec<WorkerRequest>, Vec<WorkerRequest>) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let n_out = native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect("native submit");
    let w_out = wasm
        .submit_at(block(height, origin), m)
        .await
        .expect("wasm submit");
    assert_eq!(
        event_tuples(&n_out),
        event_tuples(&w_out),
        "event traces diverge at {height}"
    );
    let (n_reqs, w_reqs) = (worker_requests(&n_out), worker_requests(&w_out));
    assert_eq!(
        n_reqs, w_reqs,
        "decoded work orders diverge at {height} — the reactor would feed workers differently"
    );
    assert_eq!(
        replies(native, ids, members).await,
        replies(wasm, ids, members).await,
        "replies diverge after block {height}"
    );
    for sibling in SIBLING_IDS {
        assert_eq!(
            native.module_root(sibling),
            wasm.module_root(sibling),
            "the {sibling} sibling diverged at {height}"
        );
    }
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
    (n_reqs, w_reqs)
}

/// submit one REJECTED op to both hosts: reasons carry the same needle, and
/// the saga roots (and every sibling) are byte-identical to pre-block — the
/// abort lane leaves no trace.
/// (one argument per invariant knob; a builder would only obscure the matrix.)
#[allow(clippy::too_many_arguments)]
async fn reject_roundtrip(
    native: &mut Host,
    wasm: &mut Host,
    ids: &[&str],
    members: &[&[u8]],
    height: u64,
    origin: Origin,
    m: Msg,
    needle: &str,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let n_err = native
        .submit_at(block(height, origin.clone()), m.clone())
        .await
        .expect_err("native must reject");
    let w_err = wasm
        .submit_at(block(height, origin), m)
        .await
        .expect_err("wasm must reject");
    let SubmitError::Rejected(Error::Module(n_msg)) = n_err else {
        panic!("native rejection shape: {n_err:?}");
    };
    let SubmitError::Rejected(Error::Module(w_msg)) = w_err else {
        panic!("wasm rejection shape: {w_err:?}");
    };
    assert!(n_msg.contains(needle), "native reason: {n_msg}");
    assert!(
        w_msg.contains(needle),
        "wasm reason must carry the native reason: {w_msg}"
    );
    assert_eq!(root_of(native), n_before, "native root moved on reject");
    assert_eq!(root_of(wasm), w_before, "wasm root moved on reject");
    for sibling in SIBLING_IDS {
        assert_eq!(native.module_root(sibling), wasm.module_root(sibling));
    }
    assert_eq!(
        replies(native, ids, members).await,
        replies(wasm, ids, members).await
    );
}

/// the (cross-host-agreed) live view of one saga on the wasm host — used to
/// read the strict lease's current assignee so results can be submitted from
/// the origin the policy accepts.
async fn view_of(h: &Host, id: &str) -> saga::SagaView {
    let reply = h
        .query(
            "saga",
            &encode_query(&SagaQuery::Get {
                saga_id: id.into(),
            }),
        )
        .await
        .expect("get");
    match decode_reply(&reply).expect("decode") {
        SagaReply::Saga(Some(v)) => v,
        other => panic!("expected a live saga, got {other:?}"),
    }
}

#[test]
fn same_ops_same_events_same_replies_roots_in_lockstep() {
    futures::executor::block_on(same_ops_inner());
}

async fn same_ops_inner() {
    let (a, b) = (key(0xAA), key(0xBB));
    let members = vec![a.clone(), b.clone()];
    let member_keys: [&[u8]; 2] = [&a, &b];
    let ids = [
        "t-open", "t-cap", "t-ann", "t-pin", "t-cxl", "t-renew", "t-re",
    ];

    let mut native = native_host(&members);
    let mut wasm = wasm_host_(&members);

    // the SCHEMA-BREAK pin, saga-shaped: the native empty root hashes the
    // empty canonical map (a bare zero count — NOT the ZERO sentinel), and the
    // wasm root hashes the empty host-KV store — the SAME 8-zero-byte
    // preimage. the genesis roots therefore COINCIDE, and the declared break
    // (revision 2) becomes visible at the FIRST committed write below.
    assert_ne!(root_of(&native), StateRoot::ZERO, "saga has no ZERO sentinel");
    assert_eq!(
        root_of(&native),
        root_of(&wasm),
        "empty-canonical-map module: genesis roots coincide by construction"
    );

    // ---- capability announcements (sibling-only blocks hold the saga root).
    let (n0, w0) = (root_of(&native), root_of(&wasm));
    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, Origin::External(a.clone())),
            announce(&["llm"], &[("cores", 8)]),
        )
        .await
        .expect("A announces");
        host.submit_at(
            block(2, Origin::External(b.clone())),
            announce(&["llm", "gpu"], &[("cores", 4)]),
        )
        .await
        .expect("B announces");
    }
    assert_eq!(root_of(&native), n0, "sibling blocks hold the native root");
    assert_eq!(root_of(&wasm), w0, "sibling blocks hold the wasm root");

    // ---- t-open: an UNTAGGED trigger draws its pool from the valset sibling
    // (a host-routed read resolved through memoized replay on the wasm side).
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        3,
        Origin::External(a.clone()),
        saga_op(&Trig {
            deadline: Some(500),
            max_attempts: 2,
            ..trigger("t-open")
        }.into()),
        true,
    )
    .await;
    assert_eq!(n_reqs.len(), 1, "one work order per trigger");
    let open_assignee = n_reqs[0].assignee.clone().expect("valset pool assigns");

    // strict lease: a NON-assignee's result is a deterministic no-op on both
    // runtimes (an accepted op that stages nothing and emits nothing).
    let stranger = members
        .iter()
        .find(|m| **m != open_assignee)
        .expect("two members")
        .clone();
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        4,
        Origin::External(stranger),
        oracle("t-open", 0, Ok(b"stolen".to_vec())),
        false,
    )
    .await;

    // the assignee's result lands: Done + the P6 callback commits into the
    // recorder IN THIS BLOCK (the recorder root moves, identically, on both).
    let (n_req_before, w_req_before) = (
        native.module_root("req"),
        wasm.module_root("req"),
    );
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        5,
        Origin::External(open_assignee),
        oracle("t-open", 0, Ok(br#"{"answer":42}"#.to_vec())),
        true,
    )
    .await;
    assert_ne!(native.module_root("req"), n_req_before, "callback landed natively");
    assert_ne!(wasm.module_root("req"), w_req_before, "callback landed through wasm");

    // ---- t-cap: a TAGGED trigger with demands draws from CapableProviders —
    // {cores: 8} narrows the llm pool to A alone, so the assignee is A on both
    // runtimes by construction.
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        6,
        Origin::External(b.clone()),
        saga_op(&Trig {
            max_attempts: 2,
            capability: Some("llm".into()),
            demands: BTreeMap::from([("cores".to_string(), 8u64)]),
            ..trigger("t-cap")
        }.into()),
        true,
    )
    .await;
    assert_eq!(
        n_reqs[0].assignee.as_deref(),
        Some(a.as_slice()),
        "the demand filter narrows the pool to A"
    );

    // an Err consumes the attempt and RE-LEASES: the retry's work order is
    // re-emitted (attempt 1) on both runtimes.
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        7,
        Origin::External(a.clone()),
        oracle("t-cap", 0, Err("transient".into())),
        true,
    )
    .await;
    assert_eq!(n_reqs.len(), 1, "the retry re-emits one work order");
    assert_eq!(n_reqs[0].attempt, 1);
    let retry_assignee = view_of(&wasm, "t-cap").await.assignee.expect("re-leased");

    // the FINAL attempt's Err lands Failed + callback (no further work order).
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        8,
        Origin::External(retry_assignee),
        oracle("t-cap", 1, Err("fatal".into())),
        true,
    )
    .await;
    assert!(n_reqs.is_empty(), "a terminal failure emits no work order");
    assert_eq!(view_of(&wasm, "t-cap").await.status, SagaStatus::Failed);

    // ---- t-ann: a tag NOBODY announced assigns nobody — the emitted work
    // order is an ANNOUNCEMENT (assignee None), and under Strict no result
    // can land until a node claims it.
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        9,
        Origin::External(a.clone()),
        saga_op(&Trig {
            capability: Some("nobody-has-this".into()),
            ..trigger("t-ann")
        }.into()),
        true,
    )
    .await;
    assert_eq!(n_reqs[0].assignee, None, "an unprovided tag assigns nobody");

    // a result for the unclaimed announcement is a strict no-op...
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        10,
        Origin::External(a.clone()),
        oracle("t-ann", 0, Ok(b"premature".to_vec())),
        false,
    )
    .await;
    // ...B claims it via Accept — the re-emitted work order names the winner —
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        11,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Accept {
            saga_id: "t-ann".into(),
            attempt: 0,
        }),
        true,
    )
    .await;
    assert_eq!(n_reqs[0].assignee.as_deref(), Some(b.as_slice()));
    // ...a LATE accept (already assigned) is a no-op...
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        12,
        Origin::External(a.clone()),
        saga_op(&SagaMsg::Accept {
            saga_id: "t-ann".into(),
            attempt: 0,
        }),
        false,
    )
    .await;
    // ...and the WINNER's result lands.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        13,
        Origin::External(b.clone()),
        oracle("t-ann", 0, Ok(b"claimed-and-done".to_vec())),
        true,
    )
    .await;

    // ---- t-pin: a static binding leases every attempt to the pinned key —
    // no pool query at all.
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        14,
        Origin::External(a.clone()),
        saga_op(&Trig {
            pinned_assignee: Some(b.clone()),
            ..trigger("t-pin")
        }.into()),
        true,
    )
    .await;
    assert_eq!(n_reqs[0].assignee.as_deref(), Some(b.as_slice()));
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        15,
        Origin::External(b.clone()),
        oracle("t-pin", 0, Ok(b"pinned-done".to_vec())),
        true,
    )
    .await;

    // ---- t-cxl: cancel is gated to the recorded trigger origin — a foreign
    // cancel is a no-op, the owner's cancel lands Cancelled + callback.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        16,
        Origin::External(a.clone()),
        saga_op(&Trig {
            max_attempts: 3,
            ..trigger("t-cxl")
        }.into()),
        true,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        17,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Cancel {
            saga_id: "t-cxl".into(),
        }),
        false,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        18,
        Origin::External(a.clone()),
        saga_op(&SagaMsg::Cancel {
            saga_id: "t-cxl".into(),
        }),
        true,
    )
    .await;

    // ---- t-renew: only the current assignee may renew, and only inside the
    // renewal window. the lease view moves identically on both runtimes.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        19,
        Origin::External(a.clone()),
        saga_op(&Trig {
            lease_views: Some(4),
            capability: Some("llm".into()),
            ..trigger("t-renew")
        }.into()),
        true,
    )
    .await;
    let renew_holder = view_of(&wasm, "t-renew").await.assignee.expect("leased");
    let stranger = members
        .iter()
        .find(|m| **m != renew_holder)
        .expect("two members")
        .clone();
    // a non-holder renew is a no-op...
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        21,
        Origin::External(stranger),
        saga_op(&SagaMsg::RenewLease {
            saga_id: "t-renew".into(),
            attempt: 0,
        }),
        false,
    )
    .await;
    // ...the holder's renew inside the half-window extends the lease (the
    // updated_at bump alone moves the root even when the expiry holds).
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        22,
        Origin::External(renew_holder),
        saga_op(&SagaMsg::RenewLease {
            saga_id: "t-renew".into(),
            attempt: 0,
        }),
        true,
    )
    .await;

    // ---- t-re: the trigger origin reassigns — the incremented attempt is
    // rendezvous-assigned over the pool MINUS the old holder, so it lands on
    // the other member, and the re-emitted work order names it.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        23,
        Origin::External(a.clone()),
        saga_op(&Trig {
            max_attempts: 3,
            capability: Some("llm".into()),
            ..trigger("t-re")
        }.into()),
        true,
    )
    .await;
    let first_holder = view_of(&wasm, "t-re").await.assignee.expect("leased");
    // a foreign reassign is a no-op (the origin gate reads the recorded
    // trigger origin, folded state on the wasm side)...
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        24,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Reassign {
            saga_id: "t-re".into(),
            attempt: 0,
        }),
        false,
    )
    .await;
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        25,
        Origin::External(a.clone()),
        saga_op(&SagaMsg::Reassign {
            saga_id: "t-re".into(),
            attempt: 0,
        }),
        true,
    )
    .await;
    assert_eq!(n_reqs.len(), 1);
    assert_eq!(n_reqs[0].attempt, 1);
    let second_holder = n_reqs[0].assignee.clone().expect("reassigned");
    assert_ne!(second_holder, first_holder, "the old holder is excluded");

    // ---- prune: terminal-only, origin-gated GC. a foreign prune and a
    // non-terminal prune skip as no-ops; the owner's prune of a terminal saga
    // removes it (the root moves, and the Get view empties identically).
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        26,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Prune {
            saga_ids: vec!["t-open".into(), "t-re".into()],
        }),
        false,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        27,
        Origin::External(a.clone()),
        saga_op(&SagaMsg::Prune {
            saga_ids: vec!["t-open".into(), "t-re".into()],
        }),
        true,
    )
    .await;
    let reply = wasm
        .query(
            "saga",
            &encode_query(&SagaQuery::Get {
                saga_id: "t-open".into(),
            }),
        )
        .await
        .expect("get");
    assert_eq!(
        decode_reply(&reply).expect("decode"),
        SagaReply::Saga(None),
        "the pruned saga is gone"
    );

    // the roots diverged at the first write and never re-converge (the
    // declared schema break, revision 2).
    assert_ne!(root_of(&native), root_of(&wasm));

    // queries are read-only on the wasm side too: the root is stable across
    // the whole read matrix.
    let settled = root_of(&wasm);
    let _ = replies(&wasm, &ids, &member_keys).await;
    assert_eq!(root_of(&wasm), settled, "a query moved the wasm root");
}

#[test]
fn crank_times_out_and_expires_leases_in_lockstep() {
    futures::executor::block_on(crank_inner());
}

async fn crank_inner() {
    let (a, b) = (key(0xAA), key(0xBB));
    let members = vec![a.clone(), b.clone()];
    let member_keys: [&[u8]; 2] = [&a, &b];
    let ids = ["t-dead", "t-lease"];

    let mut native = native_host(&members);
    let mut wasm = wasm_host_(&members);

    // a saga bounded by an absolute deadline, and one bounded by short leases.
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        1,
        Origin::External(a.clone()),
        saga_op(&Trig {
            deadline: Some(4),
            ..trigger("t-dead")
        }.into()),
        true,
    )
    .await;
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        2,
        Origin::External(a.clone()),
        saga_op(&Trig {
            lease_views: Some(2),
            max_attempts: 2,
            ..trigger("t-lease")
        }.into()),
        true,
    )
    .await;

    // a crank BEFORE anything expired stages nothing (both roots hold).
    roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        3,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Crank {}),
        false,
    )
    .await;

    // by view 5 the deadline (4) has passed and t-lease's first lease (2+2)
    // expired: ONE permissionless crank times t-dead out (callback) and
    // re-leases t-lease (a fresh work order for attempt 1) — identically.
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        5,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Crank {}),
        true,
    )
    .await;
    assert_eq!(n_reqs.len(), 1, "the expired lease re-emits one work order");
    assert_eq!(n_reqs[0].saga_id, "t-lease");
    assert_eq!(n_reqs[0].attempt, 1);
    assert_eq!(view_of(&wasm, "t-dead").await.status, SagaStatus::TimedOut);

    // the SECOND lease expires with attempts exhausted: the next crank lands
    // Failed + callback (no further work order).
    let (n_reqs, _) = roundtrip(
        &mut native,
        &mut wasm,
        &ids,
        &member_keys,
        9,
        Origin::External(b.clone()),
        saga_op(&SagaMsg::Crank {}),
        true,
    )
    .await;
    assert!(n_reqs.is_empty());
    let view = view_of(&wasm, "t-lease").await;
    assert_eq!(view.status, SagaStatus::Failed);
    assert_eq!(view.error.as_deref(), Some("lease attempts exhausted"));
}

#[test]
fn rejections_match_and_leave_no_trace() {
    futures::executor::block_on(rejections_inner());
}

async fn rejections_inner() {
    let (a, b) = (key(0xAA), key(0xBB));
    let members = vec![a.clone(), b.clone()];
    let member_keys: [&[u8]; 2] = [&a, &b];
    let ids = ["live", "solo", "pinned"];

    let mut native = native_host(&members);
    let mut wasm = wasm_host_(&members);

    // seed: a lone provider for "solo" (reassignment has no alternate), a live
    // assigned saga (the oversized-outcome seam is gated to its assignee), a
    // single-attempt saga, and a pinned saga.
    for host in [&mut native, &mut wasm] {
        host.submit_at(
            block(1, Origin::External(a.clone())),
            announce(&["solo"], &[]),
        )
        .await
        .expect("announce solo");
        host.submit_at(
            block(2, Origin::External(a.clone())),
            saga_op(&Trig {
                max_attempts: 3,
                capability: Some("solo".into()),
                ..trigger("live")
            }.into()),
        )
        .await
        .expect("trigger live");
        host.submit_at(
            block(3, Origin::External(a.clone())),
            saga_op(&Trig {
                capability: Some("solo".into()),
                ..trigger("solo")
            }.into()),
        )
        .await
        .expect("trigger solo");
        host.submit_at(
            block(4, Origin::External(a.clone())),
            saga_op(&Trig {
                max_attempts: 3,
                pinned_assignee: Some(b.clone()),
                ..trigger("pinned")
            }.into()),
        )
        .await
        .expect("trigger pinned");
    }
    // "solo"'s only provider is A, so both live sagas lease to A.
    assert_eq!(view_of(&wasm, "live").await.assignee.as_deref(), Some(a.as_slice()));

    // every distinct refusal family the native module implements. (the
    // 12 MiB spec cap shares its code path with the reply_payload cap below
    // and is exercised by saga's own unit tests — a 44 MiB JSON op would
    // slow this proof for no additional coverage.)
    let rejects: Vec<(Origin, Msg, &str)> = vec![
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                max_attempts: 0,
                ..trigger("r1")
            }.into()),
            "trigger max_attempts must be >= 1",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                reply_payload: vec![7; MAX_REPLY_PAYLOAD_BYTES + 1],
                ..trigger("r2")
            }.into()),
            "trigger reply_payload is",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                capability: Some(String::new()),
                ..trigger("r3")
            }.into()),
            "trigger capability must be non-empty",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                capability: Some("x".repeat(MAX_CAPABILITY_BYTES + 1)),
                ..trigger("r4")
            }.into()),
            "trigger capability is",
        ),
        // the shared validate_resources invariant: zero values and non-tag
        // dimension keys reject at trigger time.
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                capability: Some("llm".into()),
                demands: BTreeMap::from([("cores".to_string(), 0u64)]),
                ..trigger("r5")
            }.into()),
            "is zero",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                capability: Some("llm".into()),
                demands: BTreeMap::from([("NOT A TAG".to_string(), 1u64)]),
                ..trigger("r6")
            }.into()),
            "invalid characters",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                pinned_assignee: Some(Vec::new()),
                ..trigger("r7")
            }.into()),
            "pinned_assignee must be non-empty",
        ),
        // the callback-poison rule: a self-targeting or unknown reply_to
        // rejects at trigger time (the unknown-module check is a module-root
        // sibling read — resolved through the runtime on the wasm side).
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                reply_to: Some("saga".into()),
                ..trigger("r8")
            }.into()),
            "must not target the saga module itself",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                reply_to: Some("ghost".into()),
                ..trigger("r9")
            }.into()),
            "targets unknown module ghost",
        ),
        // oversized outcomes ABORT rather than commit into the root preimage —
        // submitted from the assignee so the size check is what rejects.
        (
            Origin::External(a.clone()),
            oracle("live", 0, Ok(vec![7; MAX_RESULT_BYTES + 1])),
            "oracle result is",
        ),
        (
            Origin::External(a.clone()),
            oracle("live", 0, Err("e".repeat(MAX_ERROR_BYTES + 1))),
            "oracle error is",
        ),
        // the Accept origin gates.
        (
            Origin::System,
            saga_op(&SagaMsg::Accept {
                saga_id: "live".into(),
                attempt: 0,
            }),
            "Accept requires an external origin",
        ),
        (
            Origin::External(Vec::new()),
            saga_op(&SagaMsg::Accept {
                saga_id: "live".into(),
                attempt: 0,
            }),
            "non-empty submitter id",
        ),
        // reassignment door checks: pinned sagas never reassign, a lone
        // provider has no alternate, and a single-attempt saga has no attempt
        // left to burn.
        (
            Origin::External(a.clone()),
            saga_op(&SagaMsg::Reassign {
                saga_id: "pinned".into(),
                attempt: 0,
            }),
            "pinned saga cannot be reassigned",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&SagaMsg::Reassign {
                saga_id: "solo".into(),
                attempt: 0,
            }),
            "reassignment attempts exhausted",
        ),
        (
            Origin::External(a.clone()),
            saga_op(&SagaMsg::Reassign {
                saga_id: "live".into(),
                attempt: 0,
            }),
            "no alternate assignee is available",
        ),
        // the decode seam.
        (
            Origin::External(a.clone()),
            Msg {
                target: "saga".into(),
                payload: b"definitely-not-json".to_vec(),
            },
            "expected value",
        ),
    ];

    for (height, (origin, m, needle)) in rejects.into_iter().enumerate() {
        let height = height as u64 + 5;
        reject_roundtrip(
            &mut native,
            &mut wasm,
            &ids,
            &member_keys,
            height,
            origin,
            m,
            needle,
        )
        .await;
    }
}

#[test]
fn multi_dispatch_block_reads_prior_writes_and_isolates_rejections() {
    futures::executor::block_on(multi_dispatch_inner());
}

async fn multi_dispatch_inner() {
    // ONE member: the valset pool is [A], so every rendezvous assigns A and
    // the strict gate accepts A's results — deterministic by construction.
    let a = key(0xAA);
    let members = vec![a.clone()];
    let member_keys: [&[u8]; 1] = [&a];
    let ids = ["s1", "s2"];

    let mut native = native_host(&members);
    let mut wasm = wasm_host_(&members);

    // ONE block, three ops: the result reads the STAGED trigger (and its
    // staged lease), and the prune reads the STAGED terminal state — on the
    // wasm side each later dispatch reloads the prior dispatch's staged
    // `__state` (the read-your-writes seam the adapter relies on). the P6
    // callback still fires from the middle dispatch.
    let batch = vec![
        (Origin::External(a.clone()), saga_op(&trigger("s1").into())),
        (
            Origin::External(a.clone()),
            oracle("s1", 0, Ok(b"same-block".to_vec())),
        ),
        (
            Origin::External(a.clone()),
            saga_op(&SagaMsg::Prune {
                saga_ids: vec!["s1".into()],
            }),
        ),
    ];
    let n_out = native
        .submit_block(block(1, Origin::External(a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(1, Origin::External(a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(
            out.members
                .iter()
                .all(|m| matches!(m, MemberOutcome::Applied { .. })),
            "all members must apply: {:?}",
            out.members
        );
    }
    let tuples = |events: &[sdk::Event]| -> Vec<(String, Vec<u8>)> {
        events
            .iter()
            .map(|e| (e.source.clone(), e.payload.clone()))
            .collect()
    };
    assert_eq!(tuples(&n_out.events), tuples(&w_out.events));
    assert_eq!(
        replies(&native, &ids, &member_keys).await,
        replies(&wasm, &ids, &member_keys).await
    );
    // triggered, resolved, and pruned within one block: nothing remains.
    let reply = wasm
        .query(
            "saga",
            &encode_query(&SagaQuery::Get {
                saga_id: "s1".into(),
            }),
        )
        .await
        .expect("get");
    assert_eq!(decode_reply(&reply).expect("decode"), SagaReply::Saga(None));
    // ...but the callback COMMITTED (the recorder left its empty root).
    let empty_recorder = StateRoot(sha2::Sha256::digest([]).into());
    assert_ne!(
        native.module_root("req"),
        Some(empty_recorder),
        "the same-block callback committed"
    );
    assert_eq!(native.module_root("req"), wasm.module_root("req"));

    // ONE block where the SECOND member rejects: the runtime aborts the
    // staged overlay and replays the accepted member — committed state equals
    // the accepted subset alone, on both runtimes.
    let (n_before, w_before) = (root_of(&native), root_of(&wasm));
    let batch = vec![
        (Origin::External(a.clone()), saga_op(&trigger("s2").into())),
        (
            Origin::External(a.clone()),
            saga_op(&Trig {
                max_attempts: 0,
                ..trigger("s3")
            }.into()),
        ),
    ];
    let n_out = native
        .submit_block(block(2, Origin::External(a.clone())), batch.clone())
        .await
        .expect("native block");
    let w_out = wasm
        .submit_block(block(2, Origin::External(a.clone())), batch)
        .await
        .expect("wasm block");
    for out in [&n_out, &w_out] {
        assert!(matches!(out.members[0], MemberOutcome::Applied { .. }));
        assert!(matches!(out.members[1], MemberOutcome::Rejected { .. }));
    }
    assert_ne!(root_of(&native), n_before, "the accepted member landed");
    assert_ne!(root_of(&wasm), w_before, "the accepted member landed");
    assert_eq!(
        replies(&native, &ids, &member_keys).await,
        replies(&wasm, &ids, &member_keys).await
    );
    assert!(matches!(
        decode_reply(
            &wasm
                .query(
                    "saga",
                    &encode_query(&SagaQuery::Get {
                        saga_id: "s2".into()
                    })
                )
                .await
                .expect("get")
        )
        .expect("decode"),
        SagaReply::Saga(Some(_))
    ));
}
