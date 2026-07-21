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
//! * **putblob → commit(Chunks) in one block** converges: putblob writes the
//!   staging entry into refs, which rides `REFS_KEY`, so a later dispatch's
//!   commit sees the staged chunk in `refs.staging` and reclaims it — no
//!   dependence on the block-local object index.
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
//! the one native behavior the per-dispatch model does NOT reproduce is the
//! block-local object index (`Pending::object_ids`) spanning dispatches: a chunk
//! produced INLINE by an earlier commit this block (thus in `object_ids` but NOT
//! in `refs.staging`) is availability-visible to a later same-block
//! `Content::Chunks` reference or putblob-dedup natively, but the guest's fresh
//! per-dispatch pending does not carry it. this cannot be closed without either
//! consulting the odb for availability (which finding #1 in `fs.rs` forbids —
//! the local orphan set is node-dependent) or a block-spanning guest overlay the
//! adapter model has no seam for. it is flagged in the task report; every other
//! op sequence is byte-identical.

use duckfs_core::{
    decode_msg, encode_refs, Fs, FilesMsg, ObjectStore, PUTBLOB_FRAME_TAG,
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

/// the whole per-dispatch step: apply the op, then hand off this dispatch's
/// pending exactly as the native `commit_block` does its pure step 1 — flush the
/// block's staged objects into the store (`object-put`, a staged host effect)
/// and return the new refs image to persist under [`REFS_KEY`]. `None` when the
/// op staged nothing (defensive: every verb sets a pending, so a successful
/// dispatch always returns `Some`). on a rejected op the `?` short-circuits
/// BEFORE any object flush or refs save, so the host aborts the block with
/// nothing staged — the native reject-then-abort_block sequence.
#[cfg(any(feature = "guest", test))]
pub fn dispatch<S: ObjectStore>(
    fs: &mut Fs<S>,
    ctx: &mut dyn Ctx,
    payload: &[u8],
) -> Result<Option<Vec<u8>>, Error> {
    apply_op(fs, ctx, payload)?;
    let Some((refs, _height, objects)) = fs.commit_block() else {
        return Ok(None);
    };
    // flush the block's staged objects; ordering vs. the refs save is
    // irrelevant here — the host enforces objects-before-refs at the real block
    // boundary (`StateBacking::Odb::commit_block`).
    let store = fs.store_mut();
    for (kind, body) in &objects {
        store.put(*kind, body).map_err(Error::Module)?;
    }
    Ok(Some(encode_refs(&refs)))
}

// ============================================================================
// FilesGuest — the wasm entry surface Task 4's `files-wasm` wrapper forwards to
// ============================================================================

#[cfg(feature = "guest")]
mod entry {
    use super::{dispatch, REFS_KEY};
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

        /// dispatch one op: load refs, apply, then stage the new refs image as an
        /// OUTER host write (the object flush already rode `object-put` inside
        /// `dispatch`). the host publishes both at the block boundary or discards
        /// them on abort.
        pub fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
            let mut fs = Self::load()?;
            let mut ctx = WitCtx::new();
            // NOTE: no `block_on` — every `Fs` verb is synchronous (unlike the
            // native `Module::execute` async signature the store-backed guests
            // drive), so the guest applies the op straight-line.
            let image = dispatch(&mut fs, &mut ctx, &payload).map_err(to_wit_error)?;
            if let Some(image) = image {
                host::state_set(REFS_KEY, &image);
            }
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
    use super::dispatch;
    use duckfs_core::objects::object_id;
    use duckfs_core::{
        decode_refs, encode_msg, encode_putblob, encode_refs, to_hex, Change, Content, Fs,
        FilesMsg, Kind, MemStore, ObjectId, ObjectStore, Refs,
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

    /// happy path: putblob then commit(Chunks) across TWO dispatches, chaining
    /// through the refs image on `REFS_KEY` — and the resulting image is
    /// BYTE-IDENTICAL to the native block-boundary refs (root continuity in
    /// miniature; the debuggable seed for Task 5's cross-runtime parity).
    #[test]
    fn putblob_then_commit_chains_refs_lane_byte_identical_to_native() {
        let chunk = b"hello duckfs";
        let chunk_hex = to_hex(&object_id(Kind::Chunk, chunk));
        let store = SharedMem::default();

        // ---- guest lane: refs round-trips through the state key per dispatch --
        let img1 = {
            let mut fs = Fs::new(store.clone(), Refs::default());
            dispatch(&mut fs, &mut ctx_at(1, 1), &encode_putblob(chunk))
                .unwrap()
                .expect("putblob stages a pending")
        };
        let img2 = {
            let refs = decode_refs(&img1).unwrap();
            let mut fs = Fs::new(store.clone(), refs);
            dispatch(
                &mut fs,
                &mut ctx_at(1, 1),
                &commit_chunks("/a", chunk.len() as u64, &chunk_hex),
            )
            .unwrap()
            .expect("commit stages a pending")
        };

        // ---- native twin: one Fs, block-boundary commit_block+adopt ----------
        let mut native = Fs::new(MemStore::new(), Refs::default());
        native.putblob("system", 1, chunk).unwrap();
        native
            .commit(
                "system",
                1,
                1,
                None,
                "c".into(),
                vec![Change::Put {
                    path: "/a".into(),
                    exec: false,
                    meta: Default::default(),
                    content: Content::Chunks {
                        size: chunk.len() as u64,
                        chunks: vec![chunk_hex.clone()],
                    },
                }],
            )
            .unwrap();
        let (refs, _h, objects) = native.commit_block().expect("block staged");
        for (kind, body) in &objects {
            native.store_mut().put(*kind, body).unwrap();
        }
        native.adopt_refs(refs);

        assert_eq!(
            img2,
            encode_refs(native.refs()),
            "guest per-dispatch refs image must byte-match the native \
             block-boundary refs — the root-continuity contract"
        );
    }

    /// rejection path: a commit referencing a chunk that was never staged (and
    /// is not produced in-block) is rejected with the availability error —
    /// finding #1's consensus-uniform verdict, reproduced verbatim by the guest.
    #[test]
    fn commit_referencing_unstaged_chunk_is_rejected() {
        let phantom = to_hex(&object_id(Kind::Chunk, b"never staged"));
        let mut fs = Fs::new(SharedMem::default(), Refs::default());
        let err = dispatch(&mut fs, &mut ctx_at(1, 1), &commit_chunks("/a", 12, &phantom))
            .expect_err("an unstaged chunk must reject");
        assert!(
            matches!(&err, Error::Module(m) if m.contains("chunk not available")),
            "expected the availability reject, got {err:?}"
        );
    }
}
