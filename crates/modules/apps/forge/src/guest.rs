//! the wasm-guest twin of [`module`](crate::module): the pure
//! [`ForgeState`] core over the host state lane, so the accept/reject logic is
//! SINGLE-SOURCED with the native module — both call the SAME
//! [`ForgeState::apply`], arm-for-arm. the host owns everything that touches a
//! git object database (`root`/`query`/`snapshot`/`install`/materialization)
//! via the kernel `StateBacking::Odb` backing (`ForgeOdbBacking`, native-only);
//! the guest owns ONLY `execute`.
//!
//! ## per-dispatch re-entry, and why it reproduces the native block
//!
//! the native module keeps ONE `ForgeState` alive across a whole block: every
//! op stages a per-branch fate (one fate per branch per block) and a tracker
//! mutation onto it, and `commit_block` publishes them. an adapter guest is
//! re-instantiated per DISPATCH, so it cannot hold a block-spanning core.
//! instead each dispatch re-enters the block from two host-lane values:
//!
//! 1. the chained STATE IMAGE under [`REFS_KEY`] — the committed image at the
//!    block's first dispatch, the previous dispatch's published image after
//!    (staged-over-committed, the read-your-writes seam);
//! 2. the BLOCK SCRATCH under [`BLOCK_SCRATCH_KEY`] — every fate staged so far
//!    this block, with the committed head each shadows. the kernel drops this
//!    key at the block boundary (it is never adopted into the backing), so a
//!    fresh block's first `state-get` returns `None`.
//!
//! [`ForgeState::from_lane`] rebuilds the exact native mid-block shape from
//! the two — committed refs with the staged fates on top — so the
//! one-fate-per-branch rule and every committed-only check decide identically.
//! after the op applies, the dispatch re-stages both values and hands each
//! packed head it staged to the object plane as a [`RefTarget`] record
//! (kind [`REF_TARGET_KIND`]): the kernel delivers those to the backing at
//! commit, which turns them back into the native per-branch publish.
//!
//! on a rejected op the `?` short-circuits BEFORE any state save or object
//! put, so the host aborts the block with nothing staged — the native
//! reject-then-`abort_block` sequence.

use guest_adapter::{Guest, WitCtx, block_on, host};
use sdk::Error;

use crate::state::{
    BlockScratch, ForgeState, Image, REF_TARGET_KIND, decode_block_scratch, decode_image,
    encode_block_scratch, encode_ref_target,
};

/// the guest's state key — the chained state image, whose composed root the
/// host derives the module root from (`StateBacking::Odb`). MUST equal
/// `wasm_host::REFS_KEY` (`b"__state"`) — a mismatch silently forks the
/// network.
const REFS_KEY: &[u8] = b"__state";

/// the guest's EPHEMERAL sibling of [`REFS_KEY`]: the block scratch, staged
/// each dispatch and DROPPED at the block boundary by the kernel (never
/// adopted into the backing). it carries no consensus weight — the root is the
/// image's composition — it only lets a per-dispatch guest reproduce the
/// native block-spanning staging.
const BLOCK_SCRATCH_KEY: &[u8] = b"__block_refs";

/// where issue/PR discussion follow-ups go — the chat module every production
/// node registers beside forge (the native lanes pass the same id).
const CHAT_MODULE: &str = "chat";

/// map an inner sdk error onto the wit surface — `Module` is the native
/// rejection verbatim (the INVERSE of the host's `to_wit_error`), so a
/// rejection reads identically whether forge ran native or wasm.
fn to_wit_error(e: Error) -> host::Error {
    match e {
        Error::Module(m) => host::Error::Rejected(m),
        other => host::Error::Rejected(other.to_string()),
    }
}

/// re-enter the block: the chained image (missing = genesis, empty) and the
/// scratch so far (missing = the block's first dispatch). a malformed value
/// is host-store corruption surfaced as a deterministic reject, never a
/// silent re-genesis (which would wipe the module).
fn load() -> Result<(ForgeState, BlockScratch), host::Error> {
    let image = match host::state_get(REFS_KEY) {
        None => Image::default(),
        Some(bytes) => decode_image(&bytes).map_err(to_wit_error)?,
    };
    let scratch = match host::state_get(BLOCK_SCRATCH_KEY) {
        None => BlockScratch::default(),
        Some(bytes) => decode_block_scratch(&bytes).map_err(to_wit_error)?,
    };
    let state = ForgeState::from_lane(image, scratch.clone());
    Ok((state, scratch))
}

/// dispatch one op: re-enter the block, apply through the shared core, then
/// hand this dispatch's packed heads to the object plane and re-stage the
/// chained image + scratch as OUTER host writes. the host adopts the image at
/// the block boundary (dropping the scratch) or discards both on abort.
fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
    let (mut state, before) = load()?;
    let mut ctx = WitCtx::new();
    block_on(state.apply(&mut ctx, &payload, Some(CHAT_MODULE))).map_err(to_wit_error)?;
    for target in state.ref_targets_since(&before) {
        host::object_put(REF_TARGET_KIND, &encode_ref_target(&target));
    }
    host::state_set(REFS_KEY, &state.published_image());
    host::state_set(
        BLOCK_SCRATCH_KEY,
        &encode_block_scratch(&state.block_scratch()),
    );
    Ok(())
}

/// the `ducktape:module` component export. the packaging cdylib around this
/// crate is synthesized by `guest-builder` — this export is the whole of the
/// guest's entry wiring.
struct Component;

impl Guest for Component {
    fn execute(payload: Vec<u8>) -> Result<(), host::Error> {
        execute(payload)
    }

    /// UNREACHABLE for the odb backing: the kernel serves `query` host-side
    /// from the committed maps + the git odb and early-returns `backing.query`
    /// WITHOUT instantiating the guest (`StateBacking::Odb`). fail loud rather
    /// than fabricate an answer — a deterministic error, identical on every
    /// validator, if the host ever wires it wrong.
    fn query(_req: Vec<u8>) -> Result<Vec<u8>, host::Error> {
        Err(host::Error::Unsupported)
    }
}

guest_adapter::export_module!(Component);
