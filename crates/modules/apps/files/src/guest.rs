//! the wasm-guest twin of [`module`](crate::module): [`FilesGuest`] runs the
//! pure [`duckfs_core::Fs`] over the host object plane ([`GuestOdb`]), so the op
//! semantics are SINGLE-SOURCED with the native module — both delegate to the
//! SAME `Fs` methods, arm-for-arm. the host owns everything disk- and
//! read-surface-shaped (`root`/`query`/`snapshot`/`install`/`serve_sync`) via
//! the kernel `StateBacking::Odb` backing; the guest owns ONLY `execute`.
//!
//! ## per-dispatch full-apply, and why it reproduces the native block boundary
//!
//! the native module keeps ONE `Fs` alive across a whole block: every op stages
//! into one `pending` overlay, and `commit_block` at the block boundary flushes
//! the block's objects, saves the refs file, and adopts the new refs (the root
//! moves there and nowhere else). an adapter guest, by contrast, is
//! re-instantiated per DISPATCH (`module.wit`: "owns no durable memory across
//! dispatches"), so it cannot hold a block-spanning `pending`. instead each
//! dispatch FULLY applies its own op:
//!
//! 1. load the committed-or-staged refs image from the host `state-*` lane
//!    under [`REFS_KEY`] (staged-over-committed: a later dispatch reads an
//!    earlier one's staged refs, the read-your-writes seam) and rebuild `Fs`,
//! 2. run the op against a fresh per-dispatch `pending` (the SAME `Fs` verb the
//!    native `execute` calls),
//! 3. hand the pending off with `Fs::commit_block` (its PURE step — no root
//!    movement), flush the block's staged objects through [`GuestOdb`]
//!    (`object-put`, a staged host effect), and save the new refs image back
//!    under [`REFS_KEY`] (a staged host write).
//!
//! the OUTER staging — the host's per-block object overlay + state overlay — is
//! the only durable seam: the host publishes objects then adopts the refs image
//! at the REAL block boundary with duckfs's durability ordering (objects → sync
//! → refs; `StateBacking::Odb::commit_block`) or discards both on abort. the
//! observable semantics are preserved point for point:
//!
//! * **refs progression is identical.** each op's effect on refs is a pure
//!   function of `(prior refs, op)`; the guest chains refs through `REFS_KEY`
//!   exactly as the native module chains through its live `pending.refs`, so the
//!   block-boundary refs image — hence `root() = sha256(encode_refs)` — is
//!   byte-identical (the root-continuity contract).
//! * **the block-local object index reproduces native.** the native module
//!   carries `Pending::object_ids` (the block's buffered objects) in-memory
//!   across a whole block; a later same-block op's availability/dedup reads it.
//!   the per-dispatch guest re-seeds this index each dispatch from the
//!   staged-only [`BLOCK_OBJECTS_KEY`] (`__block_objects`) state key — persisted
//!   after each dispatch, dropped by the kernel at the block boundary — so a
//!   chunk produced INLINE by an earlier same-block commit (in the index but NOT
//!   in `refs.staging`) is availability-visible to a later `Content::Chunks`
//!   reference or putblob-dedup EXACTLY as native (`Fs::seed_block_objects` /
//!   `Fs::block_objects`, the additive core seam).
//! * **putblob → commit(Chunks) in one block** converges through `refs.staging`
//!   (which rides `REFS_KEY`) independently of the object index.
//! * **in-block snapshot chaining** (a later commit bases on an earlier commit's
//!   snapshot this block) converges: the earlier snapshot was `object-put` into
//!   the block overlay, so the later commit resolves it through `GuestOdb`
//!   (`object-get`) — a READ, which legitimately consults the odb.
//! * **gc / window slide are NOT block-boundary root movers.** gc is
//!   consensus-neutral (`Fs::gc` removes unreachable objects only, never touches
//!   refs, never moves the root) and is HOST-side here (it needs
//!   `list`/`remove`, which `GuestOdb` refuses by design); the bounded-window
//!   slide happens INSIDE `commit_apply` per commit op, so the guest reproduces
//!   it per dispatch. neither needs to "see the whole block".
//!
//! with the object index reconstructed, EVERY op sequence produces a
//! byte-identical block-boundary refs image — no known divergence remains.

use std::collections::BTreeMap;

use duckfs_core::{
    decode_block_objects, decode_msg, encode_block_objects, encode_refs, Fs, FilesMsg, ObjectStore,
    PUTBLOB_FRAME_TAG,
};
use sdk::{Ctx, Error, Msg, Origin};

/// the guest's SINGLE state key — the raw refs image, whose `sha256` the host
/// derives the module root from (`StateBacking::Odb`). there is NO `__root`
/// twin (unlike guest-adapter's `SnapshotBytes` tenants): the host owns the
/// root derivation, so the guest persists the image alone. MUST equal
/// `wasm_host::REFS_KEY` (`b"__state"`) — a mismatch silently forks the network.
/// guest-only: the native test drives `dispatch` directly and never touches the
/// host state lane.
#[cfg(feature = "guest")]
const REFS_KEY: &[u8] = b"__state";

/// the guest's EPHEMERAL sibling of [`REFS_KEY`]: the per-block object index
/// (`Fs::block_objects`), staged each dispatch and DROPPED at the block boundary
/// by the kernel (`StateBacking::Odb::commit_block`'s `staged.clear()` — it is
/// never adopted into the backing, so committed state never holds it and a fresh
/// block's first `state-get` returns `None`). it carries no consensus weight:
/// the root is `sha256(refs_bytes)`, which ignores the KV entirely. sanctioned
/// as the block-local scratch that lets a per-dispatch guest reproduce native's
/// in-memory `Pending::object_ids`.
#[cfg(feature = "guest")]
const BLOCK_OBJECTS_KEY: &[u8] = b"__block_objects";

/// decode the op and delegate to the SAME [`Fs`] verb the native
/// [`module`](crate::module) `execute` calls — arm-for-arm, no re-implemented
/// decision. the acting identity is origin-derived (never the payload), exactly
/// as native. generic over the store so the native test can drive it against an
/// in-memory odb; the guest drives it against [`GuestOdb`].
#[cfg(any(feature = "guest", test))]
fn apply_op<S: ObjectStore>(
    fs: &mut Fs<S>,
    ctx: &mut dyn Ctx,
    payload: &[u8],
) -> Result<(), Error> {
    let env = ctx.env().clone();
    // the acting identity is origin-derived, never taken from the payload.
    let actor = env.origin.actor_string();
    // system maps to a module origin: it may register a watch for ANY module_id
    // (the watch gate's `actor == "system"` branch), so it must be `is_module`.
    let is_module = matches!(env.origin, Origin::Module(_) | Origin::System);
    match payload.first() {
        Some(&PUTBLOB_FRAME_TAG) => fs
            .putblob(&actor, env.height, &payload[1..])
            .map_err(Error::Module),
        _ => match decode_msg(payload).map_err(Error::Module)? {
            FilesMsg::Commit {
                base_snapshot,
                message,
                changes,
            } => {
                let notifications = fs
                    .commit(
                        &actor,
                        env.height,
                        env.consensus_time,
                        base_snapshot,
                        message,
                        changes,
                    )
                    .map_err(Error::Module)?;
                // watch fan-out: each notification becomes a follow-up msg at the
                // watching module (the task-9 `duckfs_notify` JSON shape),
                // re-dispatched after execute returns — identical to native.
                for n in notifications {
                    let payload = serde_json::to_vec(&serde_json::json!({
                        "duckfs_notify": {
                            "prefix": n.prefix,
                            "path": n.path,
                            "snapshot": n.snapshot,
                        }
                    }))
                    .expect("serde_json::Value serializes");
                    ctx.emit_msg(Msg {
                        target: n.module_id,
                        payload,
                    });
                }
                Ok(())
            }
            FilesMsg::Pin { snapshot, name } => fs
                .pin(&actor, env.height, snapshot, name)
                .map_err(Error::Module),
            FilesMsg::Unpin { name } => {
                fs.unpin(&actor, env.height, name).map_err(Error::Module)
            }
            FilesMsg::Watch { prefix, module_id } => fs
                .watch(&actor, env.height, is_module, prefix, module_id)
                .map_err(Error::Module),
            FilesMsg::Unwatch { prefix, module_id } => fs
                .unwatch(&actor, env.height, is_module, prefix, module_id)
                .map_err(Error::Module),
        },
    }
}

/// the two staged host writes one guest dispatch produces: the new refs image
/// (under [`REFS_KEY`] — the root preimage) and the new block-object index
/// (under `__block_objects` — ephemeral, dropped at the block boundary).
#[cfg(any(feature = "guest", test))]
pub struct Dispatched {
    pub refs_image: Vec<u8>,
    pub block_objects: Vec<u8>,
}

/// the whole per-dispatch step. an adapter guest is rebuilt per dispatch, so it
/// must reconstruct the block-local object index the native module carries
/// in-memory across a block:
///
/// 1. seed the block-object index (decoded from the caller's `__block_objects`
///    value, empty at the block's first dispatch) into a fresh pending, so a
///    later same-block op's availability/dedup sees an earlier dispatch's
///    staged objects — root-continuity parity with native (a `Content::Chunks`
///    or putblob referencing an INLINE chunk from an earlier same-block commit);
/// 2. apply the op (its `require_pending` no-ops onto the seeded pending);
/// 3. hand off this dispatch's pending exactly as the native `commit_block`
///    does its pure step 1 — flush the block's staged objects into the store
///    (`object-put`, a staged host effect) — and return BOTH the new refs image
///    (`REFS_KEY`) and the updated block index (`__block_objects`) to re-stage.
///
/// on a rejected op the `?` short-circuits BEFORE any flush or state save, so
/// the host aborts the block with nothing staged — the native
/// reject-then-`abort_block` sequence (the whole block's `__block_objects` and
/// refs stage are discarded together).
#[cfg(any(feature = "guest", test))]
pub fn dispatch<S: ObjectStore>(
    fs: &mut Fs<S>,
    ctx: &mut dyn Ctx,
    payload: &[u8],
    block_objects: Option<&[u8]>,
) -> Result<Dispatched, Error> {
    let height = ctx.env().height;
    let index = match block_objects {
        None => BTreeMap::new(),
        Some(bytes) => decode_block_objects(bytes).map_err(Error::Module)?,
    };
    fs.seed_block_objects(height, index);
    apply_op(fs, ctx, payload)?;
    // the updated index (prior-dispatch + this dispatch) to re-stage; read
    // before commit_block takes the pending.
    let block_objects = encode_block_objects(&fs.block_objects());
    // seed_block_objects always set a pending, so commit_block yields Some.
    let (refs, _height, objects) = fs
        .commit_block()
        .expect("seed_block_objects set a pending");
    // flush this dispatch's staged objects; ordering vs. the refs save is
    // irrelevant here — the host enforces objects-before-refs at the real block
    // boundary (`StateBacking::Odb::commit_block`).
    let store = fs.store_mut();
    for (kind, body) in &objects {
        store.put(*kind, body).map_err(Error::Module)?;
    }
    Ok(Dispatched {
        refs_image: encode_refs(&refs),
        block_objects,
    })
}

// ============================================================================
// FilesGuest — the wasm entry surface Task 4's `files-wasm` wrapper forwards to
// ============================================================================

#[cfg(feature = "guest")]
mod entry {
    use super::{dispatch, BLOCK_OBJECTS_KEY, REFS_KEY};
    use duckfs_core::{decode_refs, Fs, Refs};
    use guest_adapter::{host, GuestOdb, WitCtx};
    use sdk::Error;

    /// map an inner sdk error onto the wit surface — `Module` is the native
    /// rejection verbatim (the INVERSE of the host's `to_wit_error`), so a
    /// rejection reads identically whether files ran native or wasm.
    fn to_wit_error(e: Error) -> host::Error {
        match e {
            Error::Module(m) => host::Error::Rejected(m),
            other => host::Error::Rejected(other.to_string()),
        }
    }

    /// the wasm-facing entry surface. all object I/O rides [`GuestOdb`]; the
    /// refs image rides the host `state-*` lane under [`REFS_KEY`]. a zero-sized
    /// façade so Task 4's `Guest` impl is a two-line forward.
    pub struct FilesGuest;

    impl FilesGuest {
        /// build the per-dispatch `Fs` from the committed-or-staged refs image.
        /// a missing key = genesis (empty refs); a malformed image is host-store
        /// corruption surfaced as a deterministic reject, never a silent
        /// re-genesis (which would wipe the module).
        fn load() -> Result<Fs<GuestOdb>, host::Error> {
            let refs = match host::state_get(REFS_KEY) {
                None => Refs::default(),
                Some(bytes) => decode_refs(&bytes).map_err(|e| {
                    host::Error::Rejected(format!("files: refs image decode: {e}"))
                })?,
            };
            Ok(Fs::new(GuestOdb, refs))
        }

        /// dispatch one op: load refs + the block-object index, apply, then
        /// re-stage BOTH as OUTER host writes (the object flush already rode
        /// `object-put` inside `dispatch`). the host publishes the refs at the
        /// block boundary (dropping `__block_objects`) or discards both on abort.
        pub fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
            let mut fs = Self::load()?;
            let mut ctx = WitCtx::new();
            let block_objects = host::state_get(BLOCK_OBJECTS_KEY);
            // NOTE: no `block_on` — every `Fs` verb is synchronous (unlike the
            // native `Module::execute` async signature the store-backed guests
            // drive), so the guest applies the op straight-line.
            let out = dispatch(&mut fs, &mut ctx, &payload, block_objects.as_deref())
                .map_err(to_wit_error)?;
            host::state_set(REFS_KEY, &out.refs_image);
            host::state_set(BLOCK_OBJECTS_KEY, &out.block_objects);
            Ok(())
        }

        /// UNREACHABLE for the odb backing: the kernel serves `query` host-side
        /// from committed refs + the disk odb (content bodies cannot be read in a
        /// sealed round) and early-returns `backing.query` WITHOUT instantiating
        /// the guest (`StateBacking::Odb`). fail loud rather than fabricate a
        /// body-less answer — a deterministic error, identical on every
        /// validator, if the host ever wires it wrong.
        pub fn query(_req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
            Err(host::Error::Unsupported)
        }
    }
}

#[cfg(feature = "guest")]
pub use entry::FilesGuest;

#[cfg(test)]
mod tests {
    use super::{dispatch, Dispatched};
    use base64::Engine as _;
    use duckfs_core::objects::object_id;
    use duckfs_core::{
        encode_msg, encode_putblob, encode_refs, to_hex, Change, Content, Fs, FilesMsg, Kind,
        MemStore, ObjectId, ObjectStore, Refs,
    };
    use sdk::{Ctx, Env, Error, Event, Msg, Origin, StateRoot};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// a shared in-memory odb handle — the native twin of `GuestOdb`'s
    /// statelessness: the object bytes live outside the per-dispatch `Fs` (in the
    /// guest, host-side; here, in the shared `MemStore`), so rebuilding `Fs` each
    /// dispatch keeps the accumulated objects, exactly as `object-put` in one
    /// dispatch is visible to `object-get` in the next.
    #[derive(Clone, Default)]
    struct SharedMem(Rc<RefCell<MemStore>>);

    impl ObjectStore for SharedMem {
        fn put(&mut self, kind: Kind, body: &[u8]) -> Result<ObjectId, String> {
            self.0.borrow_mut().put(kind, body)
        }
        fn get(&self, id: &ObjectId) -> Result<Option<(Kind, Vec<u8>)>, String> {
            self.0.borrow().get(id)
        }
        fn has(&self, id: &ObjectId) -> bool {
            self.0.borrow().has(id)
        }
        fn stat(&self, id: &ObjectId) -> Result<Option<(Kind, u64)>, String> {
            self.0.borrow().stat(id)
        }
        fn remove(&mut self, id: &ObjectId) -> Result<(), String> {
            self.0.borrow_mut().remove(id)
        }
        fn list(&self) -> Result<Vec<ObjectId>, String> {
            self.0.borrow().list()
        }
    }

    /// a minimal `sdk::Ctx`: a fixed dispatch env + a capture buffer for emitted
    /// follow-up msgs. no sibling lane (files never queries siblings in
    /// execute), so `query`/`module_root` are stubs.
    struct TestCtx {
        env: Env,
        msgs: Vec<Msg>,
    }

    #[async_trait::async_trait(?Send)]
    impl Ctx for TestCtx {
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
        fn emit_event(&mut self, _ev: Event) {}
    }

    fn ctx_at(height: u64, time: u64) -> TestCtx {
        TestCtx {
            env: Env {
                height,
                consensus_time: time,
                origin: Origin::System,
                me: "files".into(),
                protocol_version: 0,
            },
            msgs: Vec::new(),
        }
    }

    /// drives the guest lane across dispatches EXACTLY as the runtime does:
    /// threads the refs image (`__state`) and the block-object index
    /// (`__block_objects`) through per-dispatch state, against a shared in-memory
    /// odb (`GuestOdb`'s native twin). `end_block` models the kernel dropping
    /// `__block_objects` at the block boundary while keeping the adopted refs.
    struct GuestLane {
        store: SharedMem,
        refs_image: Option<Vec<u8>>,
        block_objects: Option<Vec<u8>>,
    }

    impl GuestLane {
        fn new() -> Self {
            Self {
                store: SharedMem::default(),
                refs_image: None,
                block_objects: None,
            }
        }
        fn dispatch(&mut self, height: u64, time: u64, payload: &[u8]) -> Result<(), Error> {
            let refs = match &self.refs_image {
                None => Refs::default(),
                Some(bytes) => duckfs_core::decode_refs(bytes).unwrap(),
            };
            let mut fs = Fs::new(self.store.clone(), refs);
            let Dispatched {
                refs_image,
                block_objects,
            } = dispatch(
                &mut fs,
                &mut ctx_at(height, time),
                payload,
                self.block_objects.as_deref(),
            )?;
            self.refs_image = Some(refs_image);
            self.block_objects = Some(block_objects);
            Ok(())
        }
        /// the block boundary: the kernel's `staged.clear()` drops
        /// `__block_objects`; the adopted refs survive.
        fn end_block(&mut self) {
            self.block_objects = None;
        }
        fn refs(&self) -> Refs {
            duckfs_core::decode_refs(self.refs_image.as_ref().expect("a dispatch ran")).unwrap()
        }
    }

    fn commit_chunks(path: &str, size: u64, chunk_hex: &str) -> Vec<u8> {
        encode_msg(&FilesMsg::Commit {
            base_snapshot: None,
            message: "c".into(),
            changes: vec![Change::Put {
                path: path.into(),
                exec: false,
                meta: Default::default(),
                content: Content::Chunks {
                    size,
                    chunks: vec![chunk_hex.to_string()],
                },
            }],
        })
    }

    fn commit_inline(path: &str, content: &[u8]) -> Vec<u8> {
        encode_msg(&FilesMsg::Commit {
            base_snapshot: None,
            message: "c".into(),
            changes: vec![Change::Put {
                path: path.into(),
                exec: false,
                meta: Default::default(),
                content: Content::Inline {
                    b64: base64::engine::general_purpose::STANDARD.encode(content),
                },
            }],
        })
    }

    /// the native block twin: one `Fs`, apply every op onto ONE pending, then
    /// commit_block + flush + adopt (the pure-core `commit_block` twin). returns
    /// the committed refs — the block-boundary root preimage.
    fn native_block(ops: &[Vec<u8>]) -> Refs {
        let mut fs = Fs::new(MemStore::new(), Refs::default());
        for payload in ops {
            apply_native(&mut fs, payload).unwrap();
        }
        let (refs, _h, objects) = fs.commit_block().expect("block staged");
        for (kind, body) in &objects {
            fs.store_mut().put(*kind, body).unwrap();
        }
        fs.adopt_refs(refs);
        fs.refs().clone()
    }

    /// apply one op to a native `Fs` the way `module.rs` execute does (system
    /// origin, height/time 1) — the single-sourced verb, no guest seam.
    fn apply_native(fs: &mut Fs<MemStore>, payload: &[u8]) -> Result<(), String> {
        match payload.first() {
            Some(&duckfs_core::PUTBLOB_FRAME_TAG) => fs.putblob("system", 1, &payload[1..]),
            _ => match duckfs_core::decode_msg(payload)? {
                FilesMsg::Commit {
                    base_snapshot,
                    message,
                    changes,
                } => fs
                    .commit("system", 1, 1, base_snapshot, message, changes)
                    .map(|_| ()),
                other => panic!("test only drives commits/putblob, got {other:?}"),
            },
        }
    }

    /// happy path: putblob then commit(Chunks) across TWO dispatches in one
    /// block, chaining through the refs image — BYTE-IDENTICAL to the native
    /// block-boundary refs (root continuity in miniature).
    #[test]
    fn putblob_then_commit_chains_refs_lane_byte_identical_to_native() {
        let chunk = b"hello duckfs";
        let chunk_hex = to_hex(&object_id(Kind::Chunk, chunk));

        let mut lane = GuestLane::new();
        lane.dispatch(1, 1, &encode_putblob(chunk)).unwrap();
        lane.dispatch(1, 1, &commit_chunks("/a", chunk.len() as u64, &chunk_hex))
            .unwrap();

        let native = native_block(&[
            encode_putblob(chunk),
            commit_chunks("/a", chunk.len() as u64, &chunk_hex),
        ]);
        assert_eq!(
            encode_refs(&lane.refs()),
            encode_refs(&native),
            "guest per-dispatch refs image must byte-match the native block-boundary refs"
        );
    }

    /// rejection path: a commit referencing a chunk that was never staged (and
    /// is not produced in-block) is rejected with the availability error —
    /// finding #1's consensus-uniform verdict, reproduced verbatim by the guest.
    #[test]
    fn commit_referencing_unstaged_chunk_is_rejected() {
        let phantom = to_hex(&object_id(Kind::Chunk, b"never staged"));
        let mut lane = GuestLane::new();
        let err = lane
            .dispatch(1, 1, &commit_chunks("/a", 12, &phantom))
            .expect_err("an unstaged chunk must reject");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("chunk not available")),
            "expected the availability reject, got {err:?}"
        );
    }

    /// DIVERGENCE FIX face 1 — an inline chunk from an earlier same-block commit,
    /// referenced by a later `Content::Chunks` commit: native ACCEPTS (the chunk
    /// is in the block-local object index); the per-dispatch guest reproduces it
    /// ONLY because `__block_objects` carries that index across dispatches. RED
    /// before the fix (dispatch 2 rejected "chunk not available"); GREEN now.
    #[test]
    fn same_block_inline_chunk_referenced_by_later_chunks_commit_matches_native() {
        let content = b"small inline body";
        let chunk_hex = to_hex(&object_id(Kind::Chunk, content));

        let mut lane = GuestLane::new();
        lane.dispatch(1, 1, &commit_inline("/a", content)).unwrap();
        lane.dispatch(1, 1, &commit_chunks("/b", content.len() as u64, &chunk_hex))
            .expect("guest must ACCEPT the same-block inline-chunk reference");

        let native = native_block(&[
            commit_inline("/a", content),
            commit_chunks("/b", content.len() as u64, &chunk_hex),
        ]);
        assert_eq!(
            encode_refs(&lane.refs()),
            encode_refs(&native),
            "guest refs must byte-match native after the same-block inline-chunk reference"
        );
    }

    /// DIVERGENCE FIX face 2 — putblob dedup against the block index: an inline
    /// chunk from an earlier same-block commit is NOT re-staged by a later
    /// putblob of the same bytes (native no-ops via the block index; the guest
    /// must too, or its `refs.staging` would gain a phantom entry → different
    /// root). RED before the fix (guest staged it); GREEN now.
    #[test]
    fn same_block_putblob_dedups_against_the_block_index_matching_native() {
        let content = b"dedup me please";
        let mut lane = GuestLane::new();
        lane.dispatch(1, 1, &commit_inline("/a", content)).unwrap();
        lane.dispatch(1, 1, &encode_putblob(content)).unwrap();

        let native = native_block(&[
            commit_inline("/a", content),
            encode_putblob(content),
        ]);
        assert!(
            lane.refs().staging.is_empty(),
            "guest must dedup the putblob against the block index (no new staging entry)"
        );
        assert_eq!(
            encode_refs(&lane.refs()),
            encode_refs(&native),
            "guest refs must byte-match native (no phantom staging entry)"
        );
    }

    /// the ephemeral key is BLOCK-LOCAL: across a block boundary the index
    /// resets (kernel `staged.clear()`), so an inline chunk from a PRIOR block is
    /// NOT referenceable by hash in a later block — native rejects it, and the
    /// guest must too (asserting `end_block` models the drop correctly).
    #[test]
    fn cross_block_inline_chunk_reference_is_rejected_like_native() {
        let content = b"prior block body";
        let chunk_hex = to_hex(&object_id(Kind::Chunk, content));

        let mut lane = GuestLane::new();
        lane.dispatch(1, 1, &commit_inline("/a", content)).unwrap();
        lane.end_block();
        let err = lane
            .dispatch(2, 2, &commit_chunks("/b", content.len() as u64, &chunk_hex))
            .expect_err("a prior-block inline chunk is not referenceable by hash");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("chunk not available")),
            "expected the availability reject across the block boundary, got {err:?}"
        );
    }
}
