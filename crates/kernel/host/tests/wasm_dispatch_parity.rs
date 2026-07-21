//! the adapter-port equivalence proof for the dispatch cutover, PART 1 — the
//! block-boundary matrix. the `dispatch-wasm` component (the NATIVE `dispatch`
//! crate compiled to wasm behind `guest-adapter`) and the native
//! [`DispatchModule`] answer the SAME op sequence with IDENTICAL accept/reject
//! verdicts, their committed reads agree between blocks, and their roots move in
//! lockstep (move on a committed write, hold on an aborted one). the roots
//! THEMSELVES differ — the port persists the native canonical snapshot as one
//! host-KV value, a declared state-schema break (revision 2) — and this proof
//! pins that difference so it can never be mistaken for accidental compatibility.
//!
//! ## why the schema break shows from GENESIS here (unlike tagging)
//!
//! tagging's empty canonical encoding is a lone zero count, byte-identical to
//! the empty host-KV store, so its genesis roots COINCIDE and the break onsets
//! at the first write. dispatch's empty encoding is FOUR zero counts (recipes,
//! dispatches, mailbox, next_seq — 32 bytes) while the empty map-backed store is
//! a lone zero count (8 bytes): different preimages, so the sha256 roots differ
//! before any write. the revision-2 break is therefore TOTAL for dispatch, and
//! the matrix asserts `native_root != wasm_root` at genesis and after every op.
//!
//! ## why there are no sibling reads on the accept path
//!
//! dispatch is a SELF-CONTAINED plane: its `execute` reads only `ctx.env()` and
//! EMITS follow-ups (a saga `Trigger`, event breadcrumbs); it makes no
//! cross-module `query-module` reads, so — unlike tagging/runs — the guest fold
//! needs no memoized sibling replay. the one ctx-routed sibling read the native
//! module has (`saga_view` assignee enrichment) lives in `query_with`, which the
//! ctx-less guest query surface does not exercise; this matrix reads only the
//! plain `Module::query` surface (committed-only on both runtimes).
//!
//! ## the saga stand-in
//!
//! dispatch routes its work over saga, so both hosts carry a [`Recorder`] under
//! `"saga"` recording every delivered `Msg`. PART 1's ops are the admin surface
//! (register / update / remove) plus rejections — none of which route to saga —
//! so the recorder stays empty throughout, which is itself the claim: admin ops
//! do not touch the saga lane, and an aborted block delivers nothing. PART 2
//! (Task 3) extends this file with the dispatch → saga → delivery flow that puts
//! the recorder to work.

use host::{BlockContext, FinalizedBlock, Host, SubmitError};
use sdk::{Ctx, Error, Event, Module, ModuleId, Msg, Origin, StateRoot, StateSyncHandle};
use sha2::{Digest, Sha256};
use dispatch::{
    decode_reply, encode_msg, encode_query, DispatchMsg, DispatchQuery, DispatchReply,
    OutputContract, Routing,
};
use wasm_host::WasmModule;

/// GENERATED artifact — built from `crates/guests/dispatch-wasm` by the module
/// build target; committed (Task 1) so this proof is self-contained.
const DISPATCH_WASM: &[u8] = include_bytes!("fixtures/dispatch.component.wasm");

fn wasm_dispatch() -> WasmModule {
    WasmModule::from_bytes("dispatch", DISPATCH_WASM)
        .expect("load component")
        // adapter port persists the native canonical snapshot as one host-KV
        // value — the revision-2 declaration bin/node makes at cutover.
        .with_state_schema_revision(2)
}

/// EXACTLY the production wiring in bin/node's host state: the saga collaborator
/// id is genesis config (not committed state), so both runtimes — and the guest
/// itself — must wire the same id or the routing forks.
fn native_dispatch() -> dispatch::DispatchModule {
    dispatch::DispatchModule::new("dispatch", "saga")
}

/// the saga stand-in: a REAL sibling under `"saga"` that records every follow-up
/// `Msg` dispatch delivers to it — under the native staging contract, so an
/// aborted block leaves no trace here either — and serves its committed log via
/// its query surface. comparing recorder logs across runtimes is the
/// routing-parity claim Task 3 leans on; in PART 1 the log stays empty (admin
/// ops route nowhere), which the matrix asserts explicitly.
struct Recorder {
    id: ModuleId,
    committed: Vec<(String, Vec<u8>)>,
    staged: Vec<(String, Vec<u8>)>,
}

impl Recorder {
    fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            committed: Vec::new(),
            staged: Vec::new(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Module for Recorder {
    fn id(&self) -> ModuleId {
        self.id.clone()
    }

    fn root(&self) -> StateRoot {
        if self.committed.is_empty() {
            return StateRoot::ZERO;
        }
        let mut h = Sha256::new();
        for (who, payload) in &self.committed {
            h.update((who.len() as u64).to_le_bytes());
            h.update(who.as_bytes());
            h.update((payload.len() as u64).to_le_bytes());
            h.update(payload);
        }
        StateRoot(h.finalize().into())
    }

    async fn execute(&mut self, ctx: &mut dyn Ctx, msg: &Msg) -> Result<(), Error> {
        self.staged
            .push((ctx.env().origin.actor_string(), msg.payload.clone()));
        Ok(())
    }

    async fn query(&self, _req: &[u8]) -> Result<Vec<u8>, Error> {
        Ok(serde_json::to_vec(&self.committed).expect("serializable"))
    }

    async fn commit_block(&mut self) -> Result<(), Error> {
        self.committed.append(&mut self.staged);
        Ok(())
    }

    async fn abort_block(&mut self) -> Result<(), Error> {
        self.staged.clear();
        Ok(())
    }
}

fn native_host() -> Host {
    Host::genesis(vec![
        Box::new(native_dispatch()),
        Box::new(Recorder::new("saga")),
    ])
    .expect("genesis")
}

fn wasm_host_() -> Host {
    Host::genesis(vec![
        Box::new(wasm_dispatch()),
        Box::new(Recorder::new("saga")),
    ])
    .expect("genesis")
}

fn op(m: &DispatchMsg) -> Msg {
    Msg {
        target: "dispatch".into(),
        payload: encode_msg(m),
    }
}

fn external(key: &[u8]) -> Origin {
    Origin::External(key.to_vec())
}

/// one block's agreed context: both runtimes see the identical env. `consensus_time`
/// is height-derived, so recipe `created_at`/`updated_at` match across runtimes.
fn block(height: u64, origin: Origin) -> BlockContext {
    BlockContext {
        height,
        consensus_time: 1_000 + height,
        origin,
        protocol_version: 0,
    }
}

fn root_of(h: &Host) -> StateRoot {
    h.module_root("dispatch").expect("dispatch registered")
}

/// the saga stand-in's committed delivery log, decoded — the plane's observable
/// routing output. equal across runtimes after every block, and empty in PART 1.
async fn saga_deliveries(h: &Host) -> Vec<(String, Vec<u8>)> {
    let reply = h.query("saga", &[]).await.expect("recorder query");
    serde_json::from_slice(&reply).expect("recorder log decodes")
}

/// a committed-only `Recipe` read, decoded — the between-blocks query surface
/// (case 3). identical on both runtimes because it reads committed state alone.
async fn recipe(h: &Host, recipe_id: &str) -> DispatchReply {
    let reply = h
        .query(
            "dispatch",
            &encode_query(&DispatchQuery::Recipe {
                recipe_id: recipe_id.into(),
            }),
        )
        .await
        .expect("recipe query");
    decode_reply(&reply).expect("reply decodes")
}

fn event_tuples(events: &[Event]) -> Vec<(String, Vec<u8>)> {
    events
        .iter()
        .map(|e| (e.source.clone(), e.payload.clone()))
        .collect()
}

/// submit one op to BOTH hosts at `height`, require BOTH accept, and assert the
/// full between-blocks parity: identical dispatch traces + events, identical
/// saga-recorder deliveries, roots that move-or-hold together per `moves`, and
/// the pinned schema break (native root != wasm root).
async fn accept(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    msg: Msg,
    moves: bool,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let n_out = native
        .submit_at(block(height, origin.clone()), msg.clone())
        .await
        .expect("native must accept");
    let w_out = wasm
        .submit_at(block(height, origin), msg)
        .await
        .expect("wasm must accept");

    assert_eq!(
        n_out.dispatches, w_out.dispatches,
        "dispatch traces diverge at block {height}"
    );
    assert_eq!(
        event_tuples(&n_out.events),
        event_tuples(&w_out.events),
        "events diverge at block {height}"
    );
    assert_eq!(
        saga_deliveries(native).await,
        saga_deliveries(wasm).await,
        "saga deliveries diverge after block {height}"
    );
    if moves {
        assert_ne!(root_of(native), n_before, "native root stuck at {height}");
        assert_ne!(root_of(wasm), w_before, "wasm root stuck at {height}");
    } else {
        assert_eq!(root_of(native), n_before, "native root moved at {height}");
        assert_eq!(root_of(wasm), w_before, "wasm root moved at {height}");
    }
    assert_ne!(
        root_of(native),
        root_of(wasm),
        "the revision-2 schema break must hold at block {height}"
    );
}

/// submit one op to BOTH hosts at `height`, require BOTH reject with the native
/// module's reason (the wasm runtime wraps it in its wit-error rendering, so the
/// claim is containment), and assert the abort left NO trace: both roots
/// byte-identical to pre-block, the saga recorder unchanged on both.
async fn reject(
    native: &mut Host,
    wasm: &mut Host,
    height: u64,
    origin: Origin,
    msg: Msg,
    needle: &str,
) {
    let (n_before, w_before) = (root_of(native), root_of(wasm));
    let saga_before = saga_deliveries(native).await;

    let n_err = native
        .submit_at(block(height, origin.clone()), msg.clone())
        .await
        .expect_err("native must reject");
    let w_err = wasm
        .submit_at(block(height, origin), msg)
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

    // the abort path: staged writes discarded, no trace on either root.
    assert_eq!(root_of(native), n_before, "native root moved on reject");
    assert_eq!(root_of(wasm), w_before, "wasm root moved on reject");
    // and no delivery escaped the aborted block, on either runtime.
    assert_eq!(
        saga_deliveries(native).await,
        saga_before,
        "native saga log moved on reject"
    );
    assert_eq!(
        saga_deliveries(wasm).await,
        saga_before,
        "wasm saga log moved on reject"
    );
    assert_ne!(root_of(native), root_of(wasm), "schema break at block {height}");
}

/// a recipe with every field populated — the case-1 registration.
fn full_recipe() -> DispatchMsg {
    DispatchMsg::RegisterRecipe {
        recipe_id: "summarize".into(),
        description: "summarize a thread".into(),
        capability: "alpha".into(),
        routing: Routing::Pinned(vec![7u8; 32]),
        output_contract: OutputContract::Json,
        max_attempts: 3,
        deadline_views: Some(100),
        lease_views: Some(20),
    }
}

#[test]
fn block_boundary_matrix_verdicts_lockstep_roots_schema_break_pinned() {
    futures::executor::block_on(block_boundary_inner());
}

async fn block_boundary_inner() {
    let mut native = native_host();
    let mut wasm = wasm_host_();

    // CASE 6 (genesis leg): the schema break is visible before any write —
    // native's four-count empty encoding and the wasm store's lone-count empty
    // encoding are different preimages. (asserted again after every op below.)
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "genesis roots differ (the revision-2 schema break is total for dispatch)"
    );

    let alice = external(b"alice");
    let bob = external(b"bob");

    // CASE 1: RegisterRecipe (all fields populated) accepted on both; a
    // re-registration of the same recipe_id rejected on both.
    accept(&mut native, &mut wasm, 1, alice.clone(), op(&full_recipe()), true).await;
    reject(
        &mut native,
        &mut wasm,
        2,
        alice.clone(),
        op(&full_recipe()),
        "already exists",
    )
    .await;

    // CASE 2: UpdateRecipe by the OWNER accepted; the same update by a DIFFERENT
    // external rejected (the owner gate).
    let update = DispatchMsg::UpdateRecipe {
        recipe_id: "summarize".into(),
        description: Some("summarize, tersely".into()),
        capability: None,
        routing: None,
        output_contract: None,
        max_attempts: Some(5),
    };
    accept(&mut native, &mut wasm, 3, alice.clone(), op(&update), true).await;
    reject(&mut native, &mut wasm, 4, bob.clone(), op(&update), "not owned").await;

    // the update committed IDENTICALLY on both runtimes (a between-blocks
    // committed-only read — no lane divergence yet).
    let n_recipe = recipe(&native, "summarize").await;
    let w_recipe = recipe(&wasm, "summarize").await;
    assert_eq!(n_recipe, w_recipe, "committed recipe diverges across runtimes");
    let DispatchReply::Recipe(Some(r)) = &n_recipe else {
        panic!("recipe committed: {n_recipe:?}");
    };
    assert_eq!(r.description, "summarize, tersely", "the owner's update landed");
    assert_eq!(r.max_attempts, 5);
    assert_eq!(r.capability, "alpha", "an unset field kept its value");
    assert_eq!(r.output_contract, OutputContract::Json);

    // CASE 3: RemoveRecipe accepted; the between-blocks Recipe query then answers
    // None on BOTH runtimes.
    accept(
        &mut native,
        &mut wasm,
        5,
        alice.clone(),
        op(&DispatchMsg::RemoveRecipe {
            recipe_id: "summarize".into(),
        }),
        true,
    )
    .await;
    assert_eq!(
        recipe(&native, "summarize").await,
        DispatchReply::Recipe(None),
        "native forgot the removed recipe"
    );
    assert_eq!(
        recipe(&wasm, "summarize").await,
        DispatchReply::Recipe(None),
        "wasm forgot the removed recipe"
    );

    // CASE 4: an aborted block (an op that rejects) leaves both roots unmoved and
    // the saga recorder empty for that block — `reject` asserts exactly that.
    // removing a recipe that no longer exists is the abort trigger.
    reject(
        &mut native,
        &mut wasm,
        6,
        alice.clone(),
        op(&DispatchMsg::RemoveRecipe {
            recipe_id: "ghost".into(),
        }),
        "unknown recipe",
    )
    .await;

    // the saga lane was never touched by any admin op: empty on both, throughout.
    assert!(
        saga_deliveries(&native).await.is_empty() && saga_deliveries(&wasm).await.is_empty(),
        "admin ops must not route to saga"
    );

    // CASE 6 (final leg): the pinned break still holds after the whole matrix.
    assert_ne!(
        root_of(&native),
        root_of(&wasm),
        "the revision-2 schema break holds after the full matrix"
    );
}

/// CASE 5: the statesync joiner path — capture the finalized snapshot the wasm
/// host serves for its dispatch tenant, verify-then-adopt it into a FRESH
/// `wasm_dispatch()`, and assert the installed guest re-derives the source root.
#[test]
fn snapshot_install_round_trip() {
    futures::executor::block_on(snapshot_install_inner());
}

async fn snapshot_install_inner() {
    let mut wasm = wasm_host_();
    let alice = external(b"alice");

    // build non-trivial committed dispatch state (two recipes over two blocks).
    wasm.submit_at(block(1, alice.clone()), op(&full_recipe()))
        .await
        .expect("register");
    wasm.submit_at(
        block(2, alice),
        op(&DispatchMsg::RegisterRecipe {
            recipe_id: "classify".into(),
            description: String::new(),
            capability: "beta".into(),
            routing: Routing::Rendezvous,
            output_contract: OutputContract::Text,
            max_attempts: 1,
            deadline_views: None,
            lease_views: None,
        }),
    )
    .await
    .expect("register 2");
    let live_root = root_of(&wasm);

    // capture the finalized snapshot the host would ship a joiner.
    let snap = wasm
        .capture_finalized_snapshot(FinalizedBlock {
            height: 2,
            app_hash: wasm.app_hash(),
        })
        .expect("capture finalized snapshot");
    let entry = snap.module("dispatch").expect("dispatch in the snapshot");
    assert_eq!(entry.root, live_root, "the snapshot root is the live module root");
    let StateSyncHandle::SnapshotBytes(bytes) = &entry.state_sync else {
        panic!(
            "the dispatch guest must serve self-contained snapshot bytes, got {:?}",
            entry.state_sync
        );
    };

    // verify-then-adopt into a fresh guest: it starts empty, then re-derives the
    // exact source root through the native `install`'s root check.
    let mut fresh = wasm_dispatch();
    assert_ne!(fresh.root(), live_root, "a fresh guest starts empty");
    fresh
        .install(bytes, entry.root)
        .expect("install verifies against the root");
    assert_eq!(
        fresh.root(),
        live_root,
        "the installed guest matches the source root"
    );
}
