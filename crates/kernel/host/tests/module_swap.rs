//! The live-code-update proof: a REAL wasm module (`hello`) and the REAL code
//! registry (`modreg`) on one host, driven across a height-gated swap boundary
//! to a SECOND component (`hello-v2`, which `inc`s by 100 instead of 1).
//!
//! What must hold at the boundary `H`:
//!   * the host's out-of-block `realize_module_swaps(H, src)` fetches the
//!     committed target hash's bytes, verifies sha256, and swaps the component
//!     KEEPING the host-owned state — the module's `root()` (and thus the
//!     root-hash) is byte-identical across the swap itself;
//!   * the drain injects EXACTLY ONE System-origin modreg `Advance` in block `H`,
//!     flipping the committed active hash into the root-hash (the consensus
//!     commitment to the new code);
//!   * block `H`'s ops execute the NEW logic over the KEPT state (2 + 100, never
//!     102-from-zero and never 3);
//!   * a node lacking the bytes (or holding tampered bytes) FAILS CLOSED;
//!   * a state-sync joiner — post-activation committed state, NO pending swap
//!     left to arm — still reconciles its genesis code to the committed ACTIVE
//!     hash instead of forking on stale code.
//!
//! Fixtures are GENERATED artifacts (see `crates/guests/hello-wasm{,-v2}`),
//! committed so the proof is self-contained.

use std::collections::BTreeMap;

use futures::executor::block_on;
use sha2::Digest;

use host::{BlockContext, CodeSource, Host, LIFECYCLE_MODULE_ID};
use lifecycle::{Lifecycle, LifecycleMsg, LifecycleQuery, LifecycleReply};
use sdk::{Error, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

const HELLO_V1: &[u8] = include_bytes!("fixtures/hello.component.wasm");
const HELLO_V2: &[u8] = include_bytes!("fixtures/hello-v2.component.wasm");

/// the swap boundary used throughout: far enough past genesis to clear
/// `lifecycle::MIN_SWAP_LEAD` from the scheduling block.
const H: u64 = 10;

fn sha(bytes: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(bytes).to_vec()
}

/// the test-side `CodeSource`: a plain in-memory content-addressed map — the
/// node injects a blobstore-backed one, the proof injects this.
struct MapSource(BTreeMap<Vec<u8>, Vec<u8>>);

impl MapSource {
    fn with(components: &[&[u8]]) -> Self {
        Self(components.iter().map(|c| (sha(c), c.to_vec())).collect())
    }
}

#[async_trait::async_trait(?Send)]
impl CodeSource for MapSource {
    async fn fetch(&self, code_hash: &[u8]) -> Option<Vec<u8>> {
        self.0.get(code_hash).cloned()
    }
}

/// the one validator key the readiness gate counts in these proofs.
const MEMBER: [u8; 32] = [7; 32];

/// a host with the code registry, a real valset (one member — the readiness
/// denominator), and the wasm `hello` module (running v1), with v1 registered
/// as hello's genesis-active code.
fn host_with_wasm() -> Host {
    let mut host = Host::new();
    host.register(Box::new(Lifecycle::new(LIFECYCLE_MODULE_ID, "valset")));
    let mut valset = valset::Valset::new("valset");
    valset.insert(MEMBER.to_vec());
    host.register(Box::new(valset));
    host.register(Box::new(
        WasmModule::from_bytes("hello", HELLO_V1).expect("load v1"),
    ));
    submit(
        &mut host,
        0,
        Origin::System,
        lifecycle_msg(&LifecycleMsg::RegisterModule {
            module_id: "hello".into(),
            code_hash: sha(HELLO_V1),
        }),
    );
    host
}

fn submit(host: &mut Host, height: u64, origin: Origin, msg: Msg) {
    let ctx = BlockContext {
        height,
        consensus_time: height,
        origin,
    };
    block_on(host.submit_at(ctx, msg)).expect("block applies");
}

fn lifecycle_msg(m: &LifecycleMsg) -> Msg {
    Msg {
        target: LIFECYCLE_MODULE_ID.into(),
        payload: lifecycle::encode_msg(m),
    }
}

fn schedule_msg(activation_height: u64, code_hash: Vec<u8>) -> Msg {
    lifecycle_msg(&LifecycleMsg::ScheduleSwap {
        name: "hello-v2".into(),
        module_id: "hello".into(),
        activation_height,
        code_hash,
    })
}

/// the member's byte-receipt signal: what `code_announce` self-submits once
/// the component is verified-resident. latches the swap `ready` (R = n = 1).
fn signal_ready_msg() -> Msg {
    lifecycle_msg(&LifecycleMsg::SwapReady {
        name: "hello-v2".into(),
        module_id: "hello".into(),
    })
}

fn inc_msg() -> Msg {
    Msg {
        target: "hello".into(),
        payload: b"inc".to_vec(),
    }
}

fn count(host: &Host) -> u64 {
    let bytes = block_on(host.query("hello", b"")).expect("count query");
    u64::from_le_bytes(bytes.try_into().expect("8-byte count"))
}

fn active_hash(host: &Host) -> (Vec<u8>, bool) {
    let req = lifecycle::encode_query(&LifecycleQuery::ModuleStatus);
    let bytes = block_on(host.query(LIFECYCLE_MODULE_ID, &req)).expect("status");
    match lifecycle::decode_reply(&bytes).expect("decode") {
        LifecycleReply::ModuleStatus { modules } => {
            let m = modules.iter().find(|m| m.module_id == "hello").expect("hello entry");
            (m.active_code_hash.clone(), m.pending.is_some())
        }
        other => panic!("expected Status, got {other:?}"),
    }
}

fn realize(host: &mut Host, height: u64, src: &dyn CodeSource) -> Result<(), Error> {
    block_on(host.realize_module_swaps(height, src))
}

/// drive the WHOLE swap scenario and return the final root-hash — shared by the
/// headline proof and the cross-node determinism check.
fn run_swap_scenario() -> (Host, StateRoot) {
    let mut host = host_with_wasm();
    let src = MapSource::with(&[HELLO_V1, HELLO_V2]);

    // two v1 incs: count 2, one step each.
    submit(&mut host, 1, Origin::External(vec![7; 32]), inc_msg());
    submit(&mut host, 2, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 2, "v1 steps by 1");

    // governance-shaped schedule: swap hello -> v2 at H.
    submit(&mut host, 3, Origin::System, schedule_msg(H, sha(HELLO_V2)));
    // the (sole) member verified the bytes and signals — the swap latches
    // ready; from here activation is the height floor alone.
    submit(&mut host, 4, Origin::External(MEMBER.to_vec()), signal_ready_msg());

    // below H nothing arms: realization is a no-op on the running code.
    realize(&mut host, H - 1, &src).expect("below H is Ok");
    submit(&mut host, H - 1, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 3, "still v1 below the boundary");

    // THE BOUNDARY. realize first (block H must execute the new code), then the
    // block: its root op runs v2 logic and the drain's injected modreg Advance
    // flips the committed active hash in the same block.
    let wasm_root_before = host.module_root("hello").expect("hello root");
    let root_hash_before = host.root_hash();
    realize(&mut host, H, &src).expect("swap realizes at H");
    assert_eq!(
        host.module_root("hello").expect("hello root"),
        wasm_root_before,
        "the swap keeps the host-owned state: root is byte-identical"
    );
    assert_eq!(
        host.root_hash(),
        root_hash_before,
        "code is invisible to the root-hash: realization alone moves nothing"
    );
    // idempotent: a second realization at the same height is a no-op.
    realize(&mut host, H, &src).expect("re-realize is Ok");

    submit(&mut host, H, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 103, "v2 logic (+100) over KEPT state (3)");
    let (active, pending) = active_hash(&host);
    assert_eq!(active, sha(HELLO_V2), "Advance flipped the committed hash at H");
    assert!(!pending, "the pending slot is freed at H");

    // after the boundary: reconciliation stays a no-op, v2 keeps running.
    realize(&mut host, H + 1, &src).expect("post-H realize is Ok");
    submit(&mut host, H + 1, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 203, "v2 keeps stepping by 100");

    let final_hash = host.root_hash();
    (host, final_hash)
}

/// the headline proof: new code goes LIVE at `H` over kept state, the committed
/// active hash flips in the same block, and the swap itself never moves a root.
#[test]
fn live_swap_at_boundary_keeps_state_and_runs_new_code() {
    run_swap_scenario();
}

/// two independent nodes running the identical finalized sequence land on the
/// identical root-hash — the swap realization introduces no per-node divergence.
#[test]
fn deterministic_across_nodes() {
    let (_, a) = run_swap_scenario();
    let (_, b) = run_swap_scenario();
    assert_eq!(a, b, "same sequence, same root-hash on both nodes");
}

/// FAIL-CLOSED: a node that lacks the armed hash's bytes — or holds bytes that
/// do not match it — must refuse the boundary, leaving the running code untouched.
#[test]
fn fails_closed_on_missing_or_tampered_bytes() {
    let mut host = host_with_wasm();
    submit(&mut host, 3, Origin::System, schedule_msg(H, sha(HELLO_V2)));
    submit(&mut host, 4, Origin::External(MEMBER.to_vec()), signal_ready_msg());

    // missing bytes: the source only has v1 (this node SIGNALED honestly in
    // consensus but its local store lost the bytes — the boundary still
    // refuses rather than forking).
    let missing = MapSource::with(&[HELLO_V1]);
    let err = realize(&mut host, H, &missing).expect_err("absent bytes fail closed");
    assert!(matches!(err, Error::Module(m) if m.contains("absent")));

    // tampered bytes: v1 bytes filed under v2's hash — sha mismatch.
    let tampered = MapSource(BTreeMap::from([(sha(HELLO_V2), HELLO_V1.to_vec())]));
    let err = realize(&mut host, H, &tampered).expect_err("mismatched bytes fail closed");
    assert!(matches!(err, Error::Module(m) if m.contains("do not match")));

    // neither attempt swapped anything: the module still runs v1.
    submit(&mut host, H, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 1, "still v1 after both refused boundaries");
}

/// the state-sync joiner: it installs POST-activation committed registry state —
/// active hash already v2, NO pending swap left to arm — while its genesis wiring
/// loaded v1. reconciliation must read the committed ACTIVE hash (not just armed
/// pendings) and bring the running code to v2, or the joiner forks on stale code.
#[test]
fn statesync_joiner_reconciles_to_committed_active_hash() {
    // source node: run the full swap so its committed registry is post-activation.
    let (source, _) = run_swap_scenario();
    let (active, pending) = active_hash(&source);
    assert_eq!(active, sha(HELLO_V2));
    assert!(!pending, "post-activation: nothing pending");

    // joiner: modreg installed from the source's committed snapshot (the
    // verify-then-adopt state-sync path), wasm module freshly wired from
    // GENESIS (v1) code.
    let modreg_root = source.module_root(LIFECYCLE_MODULE_ID).expect("modreg root");
    let mut joined_modreg = Lifecycle::new(LIFECYCLE_MODULE_ID, "valset");
    // reach the committed snapshot through the module's own state-sync surface,
    // exactly as a joiner would receive it.
    let handle = {
        let snap = source
            .capture_finalized_snapshot(host::FinalizedBlock {
                height: H + 1,
                root_hash: source.root_hash(),
            })
            .expect("finalized snapshot");
        snap.modules
            .into_iter()
            .find(|m| m.id == LIFECYCLE_MODULE_ID)
            .expect("modreg snapshot")
            .state_sync
    };
    let sdk::StateSyncHandle::SnapshotBytes(bytes) = handle else {
        panic!("modreg serves snapshot bytes");
    };
    joined_modreg
        .install(&bytes, modreg_root)
        .expect("verify-then-adopt install");

    let mut joiner = Host::new();
    joiner.register(Box::new(joined_modreg));
    joiner.register(Box::new(
        WasmModule::from_bytes("hello", HELLO_V1).expect("genesis v1 code"),
    ));

    // the reconciliation the joiner runs before applying its first block: no
    // pending swap exists, yet the running code must land on the committed ACTIVE.
    let src = MapSource::with(&[HELLO_V1, HELLO_V2]);
    realize(&mut joiner, H + 2, &src).expect("joiner reconciles");
    submit(&mut joiner, H + 2, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&joiner), 100, "the joiner runs v2 (+100), not stale v1");
}

/// the receipt gate at the host seam: a scheduled swap whose readiness never
/// covered the member set does NOT arm past its height — realization is a
/// clean no-op (never a fail-closed error, never a swap) and the drain
/// injects no Advance, however far the height runs.
#[test]
fn unready_swap_never_arms_past_its_height() {
    let mut host = host_with_wasm();
    submit(&mut host, 3, Origin::System, schedule_msg(H, sha(HELLO_V2)));
    // no SignalReady: the bytes never provably reached the member set.
    let src = MapSource::with(&[HELLO_V1, HELLO_V2]);
    realize(&mut host, H + 100, &src).expect("unready is a no-op, not an error");
    submit(&mut host, H + 100, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 1, "still v1: receipt-gated swaps wait for R=n");
    let (active, pending) = active_hash(&host);
    assert_eq!(active, sha(HELLO_V1), "committed active hash untouched");
    assert!(pending, "the pending swap keeps waiting for its receipts");
}

/// INERT without the registry: a host with only the wasm module realizes nothing
/// and injects nothing — byte-identical drain on a net without modreg.
#[test]
fn inert_without_modreg() {
    let mut host = Host::new();
    host.register(Box::new(
        WasmModule::from_bytes("hello", HELLO_V1).expect("load v1"),
    ));
    let src = MapSource::with(&[HELLO_V1, HELLO_V2]);
    realize(&mut host, H, &src).expect("no registry: nothing to reconcile");
    submit(&mut host, H, Origin::External(vec![7; 32]), inc_msg());
    assert_eq!(count(&host), 1, "plain v1 behavior, no swap side-effects");
}
