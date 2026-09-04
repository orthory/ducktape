//! The post-genesis ADMISSION proof: a brand-new wasm module (`kanban`,
//! reusing the `hello` fixture bytes) joins a RUNNING network through the code
//! registry — no node release, no genesis change.
//!
//! What must hold at the admission boundary `H`:
//!   * before `H` the module does not exist anywhere: no registry entry beyond
//!     modreg's admission-pending record (empty active hash), queries fail;
//!   * `realize_module_swaps(H, src)` fetches the committed initial hash's
//!     bytes, verifies sha256, INSTANTIATES the module through the wired
//!     [`ModuleFactory`], and registers it — the root-hash grows by exactly the
//!     new module's (empty) root, identically on every node;
//!   * block `H`'s drain-injected `Advance` flips the committed active hash;
//!   * the module executes from `H` over fresh state;
//!   * a node lacking the bytes, or a host with no factory wired, FAILS
//!     CLOSED — an admission never silently half-lands.

use std::collections::BTreeMap;

use futures::executor::block_on;
use sha2::Digest;

use host::{BlockContext, CodeSource, Host, MODULES_ID, ModuleFactory};
use modules::{Modules, ModulesMsg, ModulesQuery, ModulesReply};
use sdk::{Error, Msg, Origin, StateRoot};
use wasm_host::WasmModule;

const COMPONENT: &[u8] = include_bytes!("fixtures/hello.component.wasm");

/// the admission boundary: far enough past scheduling to clear MIN_SWAP_LEAD.
const H: u64 = 10;

fn sha(bytes: &[u8]) -> Vec<u8> {
    sha2::Sha256::digest(bytes).to_vec()
}

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

    fn origin(&self) -> &'static str {
        "test_map"
    }
}

/// the node-shaped factory: admissions instantiate through the wasm runtime.
struct WasmFactory;

impl ModuleFactory for WasmFactory {
    fn instantiate(&self, id: &str, bytes: &[u8]) -> Result<Box<dyn sdk::Module>, Error> {
        Ok(Box::new(WasmModule::from_bytes(id, bytes)?))
    }
}

const MEMBER: [u8; 32] = [7; 32];

/// a host with the code registry and a one-member valset — and NO `kanban`
/// anywhere: the module this proof admits does not exist at genesis.
fn bare_host(with_factory: bool) -> Host {
    let mut host = Host::new();
    host.register(Box::new(Modules::new(
        MODULES_ID,
        Box::new(sdk_testkit::MemStore::new()),
        "valset",
    )));
    let mut valset = valset::Valset::new("valset", Box::new(sdk_testkit::MemStore::new()));
    block_on(valset.seed(MEMBER.to_vec())).expect("seed valset");
    block_on(valset.finish_seed()).expect("seed valset");
    host.register(Box::new(valset));
    if with_factory {
        host.set_module_factory(Box::new(WasmFactory));
    }
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

fn modules_msg(m: &ModulesMsg) -> Msg {
    Msg {
        target: MODULES_ID.into(),
        payload: modules::encode_msg(m),
    }
}

fn schedule_register_msg() -> Msg {
    modules_msg(&ModulesMsg::ScheduleRegister {
        name: "kanban-v1".into(),
        module_id: "kanban".into(),
        activation_height: H,
        code_hash: sha(COMPONENT),
    })
}

fn signal_ready_msg() -> Msg {
    modules_msg(&ModulesMsg::SwapReady {
        name: "kanban-v1".into(),
        module_id: "kanban".into(),
    })
}

fn inc_msg() -> Msg {
    Msg {
        target: "kanban".into(),
        payload: b"inc".to_vec(),
    }
}

fn count(host: &Host) -> u64 {
    let bytes = block_on(host.query("kanban", b"")).expect("count query");
    u64::from_le_bytes(bytes.try_into().expect("8-byte count"))
}

fn kanban_entry(host: &Host) -> Option<(Vec<u8>, bool)> {
    let req = modules::encode_query(&ModulesQuery::ModuleStatus);
    let bytes = block_on(host.query(MODULES_ID, &req)).expect("status");
    match modules::decode_reply(&bytes).expect("decode") {
        ModulesReply::ModuleStatus { modules } => modules
            .iter()
            .find(|m| m.module_id == "kanban")
            .map(|m| (m.active_code_hash.clone(), m.pending.is_some())),
        other => panic!("expected Status, got {other:?}"),
    }
}

fn realize(host: &mut Host, height: u64, src: &dyn CodeSource) -> Result<(), Error> {
    block_on(host.realize_module_swaps(height, src))
}

/// drive the WHOLE admission and return the final root-hash — shared by the
/// headline proof and the cross-node determinism check.
fn run_admission_scenario() -> (Host, StateRoot) {
    let mut host = bare_host(true);
    let src = MapSource::with(&[COMPONENT]);

    // governance-shaped admission + the member's byte-receipt signal.
    submit(&mut host, 3, Origin::System, schedule_register_msg());
    submit(&mut host, 4, Origin::External(MEMBER.to_vec()), signal_ready_msg());

    // registered-not-running: modreg carries the admission (empty active hash,
    // one pending), the host does not know the module at all.
    let (active, pending) = kanban_entry(&host).expect("admission landed");
    assert!(active.is_empty(), "no active code before the boundary");
    assert!(pending, "the pending initial code is the admission");
    assert!(
        block_on(host.query("kanban", b"")).is_err(),
        "the module does not answer before its boundary"
    );

    // below H nothing arms: realization is a no-op and the module stays absent.
    realize(&mut host, H - 1, &src).expect("below H is Ok");
    assert!(host.module_root("kanban").is_none(), "not registered below H");

    // THE BOUNDARY: realization instantiates + registers, growing the root-hash
    // by the new module's (empty) root — deterministically.
    let root_hash_before = host.root_hash();
    realize(&mut host, H, &src).expect("admission realizes at H");
    assert!(host.module_root("kanban").is_some(), "registered at H");
    assert_ne!(
        host.root_hash(),
        root_hash_before,
        "unlike a swap, an admission changes the registry set and thus the root-hash"
    );
    // idempotent: a second realization at the same height is a no-op.
    let after_first = host.root_hash();
    realize(&mut host, H, &src).expect("re-realize is Ok");
    assert_eq!(host.root_hash(), after_first, "re-realization moves nothing");

    // block H: the module executes over fresh state; the drain's injected
    // Advance flips the committed active hash in the same block.
    submit(&mut host, H, Origin::External(vec![9; 32]), inc_msg());
    assert_eq!(count(&host), 1, "fresh state, first inc");
    let (active, pending) = kanban_entry(&host).expect("entry persists");
    assert_eq!(active, sha(COMPONENT), "Advance flipped the committed hash at H");
    assert!(!pending, "the pending slot is freed at H");

    // after the boundary the module is an ordinary hot-swappable citizen.
    realize(&mut host, H + 1, &src).expect("post-H realize is Ok");
    submit(&mut host, H + 1, Origin::External(vec![9; 32]), inc_msg());
    assert_eq!(count(&host), 2);

    let final_hash = host.root_hash();
    (host, final_hash)
}

/// the smallest compliant module — two exports, no state, no events, no
/// emitted messages — admits like any other and costs the network exactly one
/// registry entry and one empty root in the root-hash: it runs over the empty
/// store, an op is accepted as a no-op that never moves that root, and a
/// query answers empty.
#[test]
fn a_module_that_touches_nothing_admits_over_the_empty_root_and_never_moves_it() {
    const NOOP: &[u8] = include_bytes!("fixtures/noop.component.wasm");
    let mut host = bare_host(true);
    let src = MapSource::with(&[NOOP]);
    submit(
        &mut host,
        3,
        Origin::System,
        modules_msg(&ModulesMsg::ScheduleRegister {
            name: "noop-v1".into(),
            module_id: "noop".into(),
            activation_height: H,
            code_hash: sha(NOOP),
        }),
    );
    submit(
        &mut host,
        4,
        Origin::External(MEMBER.to_vec()),
        modules_msg(&ModulesMsg::SwapReady {
            name: "noop-v1".into(),
            module_id: "noop".into(),
        }),
    );
    realize(&mut host, H, &src).expect("admission realizes at H");
    let (_, empty_root) = wasm_host::initial_state(&[]);
    assert_eq!(
        host.module_root("noop"),
        Some(empty_root),
        "admitted over the empty store"
    );

    let any_op = Msg {
        target: "noop".into(),
        payload: b"anything".to_vec(),
    };
    submit(&mut host, H, Origin::External(vec![9; 32]), any_op);
    assert_eq!(
        host.module_root("noop"),
        Some(empty_root),
        "an accepted op moves nothing"
    );
    let reply = block_on(host.query("noop", b"anything")).expect("a query answers");
    assert!(reply.is_empty(), "the answer is empty, got {reply:?}");
}

/// the headline proof: a module that did not exist at genesis goes LIVE at `H`
/// through governance-shaped ops alone.
#[test]
fn admission_at_boundary_instantiates_and_runs_the_new_module() {
    run_admission_scenario();
}

/// two independent nodes running the identical finalized sequence land on the
/// identical root-hash — admission introduces no per-node divergence.
#[test]
fn admission_is_deterministic_across_nodes() {
    let (_, a) = run_admission_scenario();
    let (_, b) = run_admission_scenario();
    assert_eq!(a, b, "identical histories, identical root-hashes");
}

/// a node that does not hold the bytes at the boundary FAILS CLOSED.
#[test]
fn admission_fails_closed_on_missing_or_tampered_bytes() {
    let mut host = bare_host(true);
    submit(&mut host, 3, Origin::System, schedule_register_msg());
    submit(&mut host, 4, Origin::External(MEMBER.to_vec()), signal_ready_msg());

    let empty = MapSource::with(&[]);
    assert!(
        realize(&mut host, H, &empty).is_err(),
        "absent bytes must stop the boundary"
    );
    assert!(host.module_root("kanban").is_none(), "nothing half-landed");

    // tampered bytes: right key in the map, wrong content.
    let mut tampered = MapSource::with(&[]);
    tampered.0.insert(sha(COMPONENT), b"evil".to_vec());
    assert!(
        realize(&mut host, H, &tampered).is_err(),
        "hash-mismatched bytes must stop the boundary"
    );
    assert!(host.module_root("kanban").is_none(), "nothing half-landed");

    // and the same host still admits fine once the bytes appear.
    let src = MapSource::with(&[COMPONENT]);
    realize(&mut host, H, &src).expect("healed fetch admits");
    assert!(host.module_root("kanban").is_some());
}

/// a host with no factory wired FAILS CLOSED the moment an admission arms —
/// never before.
#[test]
fn admission_fails_closed_without_a_module_factory() {
    let mut host = bare_host(false);
    let src = MapSource::with(&[COMPONENT]);
    // inert while nothing is admitted.
    realize(&mut host, H, &src).expect("no admissions, no factory needed");

    submit(&mut host, 3, Origin::System, schedule_register_msg());
    submit(&mut host, 4, Origin::External(MEMBER.to_vec()), signal_ready_msg());
    realize(&mut host, H - 1, &src).expect("unarmed admission needs nothing");
    assert!(
        realize(&mut host, H, &src).is_err(),
        "an armed admission with no factory must stop the boundary"
    );
}

/// an admission that never latches ready never arms — however high the height.
#[test]
fn unready_admission_never_arms() {
    let mut host = bare_host(true);
    let src = MapSource::with(&[COMPONENT]);
    submit(&mut host, 3, Origin::System, schedule_register_msg());
    // no SignalReady.
    realize(&mut host, H + 100, &src).expect("unready is a no-op");
    assert!(host.module_root("kanban").is_none());
    let (active, pending) = kanban_entry(&host).expect("entry persists");
    assert!(active.is_empty());
    assert!(pending, "still waiting on readiness");
}
