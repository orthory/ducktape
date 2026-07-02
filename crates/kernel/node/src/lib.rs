//! the replication layer — a [`Node`] wraps a [`host::Host`] and gives it a
//! byte-oriented [`Transport`] seam so one host can run REPLICATED across peers.
//!
//! ## the two msg flows
//!
//! **outbound** (a locally-originated msg): [`Node::apply_local`] submits the msg
//! to the local host first (so our own view advances immediately — the "echo"),
//! then propagates the msg's bytes to peers over its [`Transport`]. this is the
//! ONLY path that ever touches the wire.
//!
//! **inbound** (a msg that arrived from a peer): [`Node::poll_inbound`] drains
//! the transport's inbound queue, decodes each batch, and submits every msg to
//! the local host — and NEVER re-propagates. that wire-level asymmetry (outbound
//! propagates, inbound does not) IS the local-only rule: it is what keeps a
//! two-node loop from ping-ponging a msg back and forth forever.
//!
//! ## why the node's re-entry rule is only wire-level
//!
//! [`host::Host::submit`] already runs the intra-block follow-up drain: a module
//! that emits a [`Msg`] via `ctx.emit_msg` has it re-dispatched as a LOCAL-ONLY
//! follow-up op (`Origin::Module`), capped at `host::MAX_DISPATCHES`, never
//! surfaced for broadcast. so module-level re-entry is already contained inside
//! one block. the node only has to enforce the rule at the network boundary:
//! ops that came off the wire are applied, not rebroadcast.
//!
//! ## pull-based, single-owner
//!
//! unlike the legacy background-task node, inbound is PULL-based
//! ([`Node::poll_inbound`]) rather than a spawned reader loop. that lets the
//! node OWN its `Host` directly (no `Arc<Mutex>`), keeps the convergence test
//! deterministic (no interval / notify race to wait on), and keeps this crate
//! runtime-agnostic — it spawns nothing and depends on no async runtime. the
//! real commonware transport (a later slice) will add its own inbound plumbing
//! behind the same [`Transport`] seam.

use std::sync::{Arc, Mutex};
use std::sync::mpsc;

use serde::{Deserialize, Serialize};

use host::{BlockContext, BlockOutcome, Host};
use sdk::{Effect, Msg, StateRoot};

/// the bytes delivered on the inbound channel: a serialized msg-batch.
pub type Inbound = Vec<u8>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("wire decode failed: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("host error: {0}")]
    Host(#[from] sdk::Error),
    /// a node-local block-boundary fault (see [`host::FatalError`]): this node's
    /// registry is indeterminate relative to its peers. the process must stop
    /// applying blocks — every lane surfaces this instead of continuing.
    #[error("{0}")]
    Fatal(host::FatalError),
}

impl From<host::SubmitError> for Error {
    fn from(e: host::SubmitError) -> Self {
        match e {
            // a deterministic rejection keeps its module-error shape.
            host::SubmitError::Rejected(e) => Error::Host(e),
            // a boundary fault keeps its fatality — callers match on this to
            // fail-stop rather than treating it as one bad op.
            host::SubmitError::Fatal(f) => Error::Fatal(f),
        }
    }
}

// ============================================================================
// wire codec — encode Msg in THIS crate (encode-in-node), not on sdk::Msg.
// ============================================================================
//
// sdk deliberately carries no serde dep ("async-trait is the one exception"), so
// the wire concern lives here rather than deriving Serialize on `Msg`. `WireMsg`
// is a private serde mirror of the two public `Msg` fields; a batch is a plain
// `Vec<WireMsg>` over serde_json. only the app-hash has to match across nodes,
// not the wire bytes, so a json envelope is free to evolve independently.

#[derive(Serialize, Deserialize)]
struct WireMsg {
    target: String,
    payload: Vec<u8>,
}

impl From<&Msg> for WireMsg {
    fn from(m: &Msg) -> Self {
        WireMsg { target: m.target.clone(), payload: m.payload.clone() }
    }
}

impl From<WireMsg> for Msg {
    fn from(w: WireMsg) -> Self {
        Msg { target: w.target, payload: w.payload }
    }
}

/// serialize a msg-batch to bytes. infallible — the fields are plain data.
pub fn encode_batch(msgs: &[Msg]) -> Vec<u8> {
    let wire: Vec<WireMsg> = msgs.iter().map(WireMsg::from).collect();
    serde_json::to_vec(&wire).expect("msg batch serializes")
}

/// deserialize a msg-batch from bytes.
pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Msg>, Error> {
    let wire: Vec<WireMsg> = serde_json::from_slice(bytes)?;
    Ok(wire.into_iter().map(Msg::from).collect())
}

// ============================================================================
// the transport seam + the in-process loopback impl.
// ============================================================================

/// byte-oriented transport seam: send an already-serialized msg-batch to peers.
///
/// `send` is async so the seam fits an over-the-wire impl (the later commonware
/// p2p transport) without changing shape; the loopback impl's body is a plain
/// synchronous push into each peer's queue wrapped in an `async move`, so it
/// never actually suspends. the inbound side is NOT on the trait: each transport
/// hands back its receiver at construction (see [`LoopbackHub::node`]), which
/// sidesteps the object-safety question and lets a caller hold the concrete
/// receiver type. (the trait is used behind a generic `T`, never `dyn`, so the
/// return-position `impl Future` is fine.)
pub trait Transport {
    /// send a serialized msg-batch out to every peer (not back to self).
    fn send(&self, bytes: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>>;
}

/// mints N connected in-memory transports. when one node sends, every OTHER
/// node's inbound receiver gets the bytes — the sender does not.
#[derive(Clone, Default)]
pub struct LoopbackHub {
    peers: Arc<Mutex<Vec<mpsc::Sender<Inbound>>>>,
}

impl LoopbackHub {
    pub fn new() -> Self {
        Self::default()
    }

    /// register a new node. returns its transport handle and inbound receiver.
    pub fn node(&self) -> (LoopbackTransport, mpsc::Receiver<Inbound>) {
        let (tx, rx) = mpsc::channel();
        let id = {
            let mut peers = self.peers.lock().expect("hub lock poisoned");
            peers.push(tx);
            peers.len() - 1
        };
        (LoopbackTransport { id, peers: self.peers.clone() }, rx)
    }
}

/// a single node's handle onto the [`LoopbackHub`]. `Clone` so it can be shared;
/// `id` is the sender's own index, skipped on fan-out so it never receives its
/// own sends.
#[derive(Clone)]
pub struct LoopbackTransport {
    id: usize,
    peers: Arc<Mutex<Vec<mpsc::Sender<Inbound>>>>,
}

impl Transport for LoopbackTransport {
    fn send(&self, bytes: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>> {
        // capture cloned handles so the returned future owns its data (no borrow
        // of `self` escapes). the body never awaits — loopback delivery is a
        // synchronous fan-out — it just satisfies the async seam.
        let peers = self.peers.clone();
        let id = self.id;
        async move {
            let peers = peers.lock().expect("hub lock poisoned");
            for (i, tx) in peers.iter().enumerate() {
                if i == id {
                    continue; // never deliver a node its own send.
                }
                // best-effort gossip: a gone peer must not fail the whole send.
                let _ = tx.send(bytes.clone());
            }
            Ok(())
        }
    }
}

// ============================================================================
// the node — a replicated wrapper over host::Host.
// ============================================================================

/// a replicated host. owns its [`Host`], a [`Transport`] handle, and the inbound
/// receiver the transport handed back at construction. generic over the concrete
/// transport `T` (no `dyn`), so the same type serves loopback today and the
/// commonware transport later.
pub struct Node<T: Transport> {
    host: Host,
    transport: T,
    inbound: mpsc::Receiver<Inbound>,
}

impl<T: Transport> Node<T> {
    /// wrap `host` with a `transport` handle and that transport's `inbound`
    /// receiver.
    pub fn new(host: Host, transport: T, inbound: mpsc::Receiver<Inbound>) -> Self {
        Self { host, transport, inbound }
    }

    /// OUTBOUND — a locally-originated msg. apply it to the local host first (the
    /// echo: our view advances without waiting on a round-trip), then propagate
    /// the msg's bytes to peers. this is the ONLY path that propagates. returns
    /// the local [`BlockOutcome`] so the caller sees the resulting app-hash.
    ///
    /// `Msg` is `Clone`, so — unlike the legacy `!Clone` op — we simply clone for
    /// the wire and submit the original; no encode-first dance, no re-decode.
    pub async fn apply_local(&mut self, msg: Msg) -> Result<BlockOutcome, Error> {
        let bytes = encode_batch(std::slice::from_ref(&msg));
        let outcome = self.host.submit(msg).await?;
        // propagate AFTER the local apply so a slow peer never stalls our block.
        let _ = self.transport.send(bytes).await;
        Ok(outcome)
    }

    /// INBOUND — drain every msg-batch the transport delivered and submit each to
    /// the local host. NEVER re-propagates: that asymmetry vs [`apply_local`] is
    /// the local-only rule. returns the count of msgs applied (0 when idle), so a
    /// test can await convergence deterministically without a wall-clock sleep.
    ///
    /// the inbound queue is drained into an owned `Vec` up front so no channel
    /// borrow is held across the `host.submit(..).await`.
    pub async fn poll_inbound(&mut self) -> Result<usize, Error> {
        let batches: Vec<Inbound> = std::iter::from_fn(|| self.inbound.try_recv().ok()).collect();
        let mut applied = 0usize;
        for bytes in batches {
            for msg in decode_batch(&bytes)? {
                self.host.submit(msg).await?;
                applied += 1;
            }
        }
        Ok(applied)
    }

    /// the current app-hash of the wrapped host.
    pub fn app_hash(&self) -> StateRoot {
        self.host.app_hash()
    }

    /// borrow the wrapped host (queries, module_root inspection, ...).
    pub fn host(&self) -> &Host {
        &self.host
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_other_nodes_receive_sender_does_not() {
        let hub = LoopbackHub::new();
        let (node0, node0_rx) = hub.node();
        let (_node1, node1_rx) = hub.node();

        futures::executor::block_on(node0.send(b"hi".to_vec())).expect("send ok");

        // node1 receives it.
        assert_eq!(node1_rx.recv().expect("node1 got msg"), b"hi");
        // node0 (the sender) does not.
        assert!(matches!(node0_rx.try_recv(), Err(mpsc::TryRecvError::Empty)));
    }

    #[test]
    fn wire_roundtrip_preserves_target_and_payload() {
        let msgs = vec![
            Msg { target: "directory".into(), payload: b"hello".to_vec() },
            Msg { target: "kv".into(), payload: vec![] },
        ];
        let decoded = decode_batch(&encode_batch(&msgs)).expect("roundtrips");
        assert_eq!(decoded.len(), 2);
        assert_eq!(decoded[0].target, "directory");
        assert_eq!(decoded[0].payload, b"hello");
        assert_eq!(decoded[1].target, "kv");
        assert!(decoded[1].payload.is_empty());
    }
}

// ============================================================================
// the ordering seam — an AGREED TOTAL ORDER over opaque op frames.
// ============================================================================
//
// ## the semantic shift (why this is NOT the gossip path above)
//
// the [`Transport`]/[`LoopbackHub`] path is GOSSIP: [`Node::apply_local`] applies
// a msg to the local host IMMEDIATELY (the echo) then fans it out. that converges
// only for order-INdependent module roots (a state-based `directory` root). a
// qmdb root is op-log/MMR-order-DEPENDENT: the same SET of ops in different
// orders yields a different root, so the instant two validators apply in
// different orders their app-hash FORKS.
//
// the fix is an AGREED TOTAL ORDER. a locally-originated msg is **NOT** applied
// on submission — that optimistic echo is exactly what forks the chain the
// moment another validator's op orders first. instead the msg is proposed into
// the order via [`Orderer::submit`], and it is applied via `host.submit` ONLY
// when [`Orderer::poll_delivered`] delivers it — in the identical sequence on
// every validator, including its originator. so even an order-dependent qmdb
// root converges.
//
// ## precondition (the honest gap vs real BFT)
//
// [`RoundOrderer`] converges because every validator accumulates the IDENTICAL
// SET of frames before draining a round, then applies a deterministic,
// node-independent total order over that set. the harness guarantees the
// identical-set precondition by handing every node the same op-set (in different
// arrival orders); a real simplex finalization stream guarantees it for free
// (every honest node observes the same finalized-view sequence). the simplex
// `Orderer` is the drop-in behind this same trait — its `submit` is store.put +
// enqueue-digest, its `poll_delivered` non-blocking-drains the finalization
// stream. that is why `submit` is async here even though the deterministic body
// never suspends.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::{
    ed25519::{PrivateKey, PublicKey, Signature},
    Signer as _, Verifier as _,
};
use sdk::Origin;

/// the signing domain for op frames. domain-separated so an op signature can
/// never double as a consensus vote, an endpoint advertisement, or any other
/// signed artifact in the system.
const FRAME_NS: &[u8] = b"ducktape:op-frame:v1";

/// a wire frame: the ordered unit. carries the submitter's ed25519 public key
/// (`origin`), a per-origin monotonic `seq` (so two intentionally identical
/// msgs are still DISTINCT frames — the order key must be tie-free), and a
/// SIGNATURE binding (origin, seq, target, payload) to the origin key: after
/// [`decode_frame`] verifies it, `Origin::External(pubkey)` is AUTHENTICATED
/// AUTHORSHIP a module (e.g. governance voting) may rely on — no validator can
/// forge another identity's op. the agreed order is the byte-lexicographic
/// sort of these frames: correctness needs ONLY that the sort be a
/// deterministic, node-independent total order over distinct frames (it is) —
/// NOT that it be `(origin, seq)`-monotonic. replay of a byte-identical frame
/// is deduplicated by the consensus lane's exactly-once digest gate; per-origin
/// nonce enforcement IN STATE is the planned successor.
#[derive(Serialize, Deserialize)]
struct Frame {
    origin: Vec<u8>,
    seq: u64,
    target: String,
    payload: Vec<u8>,
    sig: Vec<u8>,
}

/// the signed preimage: length-prefixed fields so no two (seq, target,
/// payload) triples can collide across a moving boundary.
fn frame_preimage(origin: &[u8], seq: u64, msg: &Msg) -> Vec<u8> {
    let target = msg.target.as_bytes();
    let mut out = Vec::with_capacity(8 * 3 + origin.len() + target.len() + msg.payload.len());
    out.extend_from_slice(&(origin.len() as u64).to_le_bytes());
    out.extend_from_slice(origin);
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&(target.len() as u64).to_le_bytes());
    out.extend_from_slice(target);
    out.extend_from_slice(&msg.payload);
    out
}

/// frame and SIGN a locally-originated msg for the ordered lane. the signer's
/// public key becomes the frame's origin.
pub fn encode_frame(signer: &PrivateKey, seq: u64, msg: &Msg) -> Vec<u8> {
    let origin = signer.public_key().as_ref().to_vec();
    let sig = signer.sign(FRAME_NS, &frame_preimage(&origin, seq, msg));
    let frame = Frame {
        origin,
        seq,
        target: msg.target.clone(),
        payload: msg.payload.clone(),
        sig: sig.as_ref().to_vec(),
    };
    serde_json::to_vec(&frame).expect("frame serializes")
}

/// decode a delivered frame back to a `(Origin, Msg)` the host can submit —
/// VERIFYING the signature first. a frame whose origin is not a valid ed25519
/// key or whose signature does not bind (origin, seq, target, payload) errors,
/// and the ordered drain treats that as a deterministic no-op: every honest
/// validator rejects the identical forged frame identically. the verified
/// `origin` becomes the block's root `Origin::External(pubkey)` — authorship a
/// module can trust; the `seq` is ordering/replay metadata, not surfaced.
pub fn decode_frame(bytes: &[u8]) -> Result<(Origin, Msg), Error> {
    let frame: Frame = serde_json::from_slice(bytes)?;
    let pubkey = PublicKey::decode(frame.origin.as_slice())
        .map_err(|e| Error::Host(sdk::Error::Module(format!("frame origin: {e}"))))?;
    let sig = Signature::decode(frame.sig.as_slice())
        .map_err(|e| Error::Host(sdk::Error::Module(format!("frame signature: {e}"))))?;
    let msg = Msg { target: frame.target, payload: frame.payload };
    if !pubkey.verify(FRAME_NS, &frame_preimage(&frame.origin, frame.seq, &msg), &sig) {
        return Err(Error::Host(sdk::Error::Module(
            "frame signature does not bind this op to its origin".into(),
        )));
    }
    Ok((Origin::External(frame.origin), msg))
}

/// total-order broadcast over opaque op frames. `submit` proposes a frame into
/// the agreed sequence (it does NOT apply anything locally); `poll_delivered`
/// yields the SAME sequence, in the SAME order, on EVERY validator. domain-
/// agnostic — it orders `Vec<u8>`, never `Msg` (the simplex port slots in behind
/// this exact shape; that is why `submit` is async).
pub trait Orderer {
    /// propose an opaque frame into the agreed order. no local apply.
    fn submit(&mut self, frame: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>>;
    /// the newly-ordered frames since the last call, in agreed order (may be
    /// empty), each paired with its agreed VIEW/height — the block coordinate the
    /// host stamps into `Env` (identical on every validator). non-blocking.
    fn poll_delivered(&mut self) -> Vec<(u64, Vec<u8>)>;
}

/// the deterministic agreed-order impl: accumulate a round's proposed frames,
/// then on `poll_delivered` yield them SORTED by a deterministic, node-
/// independent key (the frame bytes themselves, which lead with origin+seq).
/// every validator that accumulated the identical SET yields the byte-identical
/// SEQUENCE — so order-dependent roots converge. (this is the "sort a round's
/// accumulated ops by a deterministic key" total order; real simplex is the
/// drop-in.)
#[derive(Default)]
pub struct RoundOrderer {
    pending: Vec<Vec<u8>>,
    /// the next agreed view to stamp. monotonic across rounds, assigned per frame
    /// in delivered order — deterministic because the delivery order is (the same
    /// node-independent sort on every validator).
    next_view: u64,
}

impl RoundOrderer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Orderer for RoundOrderer {
    fn submit(&mut self, frame: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>> {
        async move {
            self.pending.push(frame);
            Ok(())
        }
    }

    fn poll_delivered(&mut self) -> Vec<(u64, Vec<u8>)> {
        let mut out = std::mem::take(&mut self.pending);
        // the agreed order: a deterministic, node-independent total order over
        // the round's distinct frames. NOT arrival order.
        out.sort();
        // stamp each frame with a monotonic agreed view. the sort makes
        // frame->view identical across validators, so height/consensus_time agree.
        out.into_iter()
            .map(|f| {
                let view = self.next_view;
                self.next_view += 1;
                (view, f)
            })
            .collect()
    }
}

/// the NEGATIVE-CONTROL orderer, behind the SAME [`Orderer`] trait: it delivers
/// each validator its frames in raw ARRIVAL order — no agreed order at all. swap
/// [`RoundOrderer`] for this in the harness and nothing else changes; two nodes
/// with opposite arrival orders then apply opposite sequences and an order-
/// dependent qmdb root FORKS. that swap-only divergence is what proves the agreed
/// order is load-bearing, not decoration.
#[derive(Default)]
pub struct ArrivalOrderer {
    pending: Vec<Vec<u8>>,
    /// per-frame ascending view, arrival-ordered — deliberately NOT node-agreed
    /// (this is the negative control; opposite arrival -> opposite view stamps).
    next_view: u64,
}

impl ArrivalOrderer {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Orderer for ArrivalOrderer {
    fn submit(&mut self, frame: Vec<u8>) -> impl std::future::Future<Output = Result<(), Error>> {
        async move {
            self.pending.push(frame);
            Ok(())
        }
    }

    fn poll_delivered(&mut self) -> Vec<(u64, Vec<u8>)> {
        // NO sort: raw arrival order is exactly the no-agreed-order fork the
        // total order prevents.
        std::mem::take(&mut self.pending)
            .into_iter()
            .map(|f| {
                let view = self.next_view;
                self.next_view += 1;
                (view, f)
            })
            .collect()
    }
}

/// a replicated host on the ORDERED lane. owns its [`Host`] and an [`Orderer`].
/// unlike [`Node`], `submit` does NOT apply to the local host — it only proposes
/// into the agreed order; application happens exclusively in [`OrderedNode::
/// drain_delivered`], in the order the [`Orderer`] delivers, identically on every
/// validator. generic over the concrete orderer `O` (no `dyn`), so the same type
/// serves the deterministic orderer today and the simplex orderer later.
pub struct OrderedNode<O: Orderer> {
    host: Host,
    orderer: O,
    /// effects surfaced by every block APPLIED via `drain_delivered` and not yet
    /// taken. the host itself ignores its effect sink; on the ordered lane this is
    /// where the reactor's worker driver reads finalized `WorkerRequest`s from
    /// (via `take_effects`). accumulates in agreed-delivery order.
    effects: Vec<Effect>,
    /// the latest APPLIED consensus boundary: the last drained APP HEIGHT
    /// (`view_base + engine view`) plus the app-hash after that drain settled.
    /// this is what a state-sync service serves from
    /// (`host::Host::capture_finalized_snapshot` demands exactly this pair) —
    /// `None` until the first frame applies.
    finalized: Option<host::FinalizedBlock>,
    /// the app-height offset of the CURRENT engine's view 0. epoch cutover
    /// respawns the engine with views restarting at 0; the base keeps `Env`
    /// heights and the finalized boundary monotone across epochs
    /// (`app_height = view_base + view` — the orchestrator's epoch_base).
    view_base: u64,
    /// the last ENGINE-relative view drained (what the valset orchestrator
    /// observes and compares cutover views against). reset on epoch respawn.
    last_engine_view: Option<u64>,
    /// the deterministic CUTOVER CEILING: frames finalized at or past this
    /// ENGINE view are DISCARDED, not applied. every honest node discards by
    /// the same agreed rule, so a straggler op that finalizes on only some
    /// nodes while engines are being torn down can never fork app state —
    /// submitters resubmit in the new epoch.
    view_ceiling: Option<u64>,
}

impl<O: Orderer> OrderedNode<O> {
    pub fn new(host: Host, orderer: O) -> Self {
        Self {
            host,
            orderer,
            effects: Vec::new(),
            finalized: None,
            view_base: 0,
            last_engine_view: None,
            view_ceiling: None,
        }
    }

    /// EPOCH CUTOVER: replace the orderer (dropping the old one aborts its
    /// engine) and rebase app heights at `view_base` (the cutover app height —
    /// the orchestrator's epoch_base). clears the ceiling and the
    /// engine-relative view; the finalized boundary carries over.
    ///
    /// call this only after a final [`OrderedNode::drain_delivered`] under the
    /// ceiling — anything the old engine finalized past the ceiling was
    /// deterministically discarded on every honest node.
    pub fn cutover(&mut self, orderer: O, view_base: u64) {
        self.orderer = orderer;
        self.view_base = view_base;
        self.last_engine_view = None;
        self.view_ceiling = None;
        // effects of pre-cutover blocks remain takeable; frames buffered in
        // the OLD orderer die with it (they were past the ceiling or already
        // drained).
    }

    /// set the deterministic discard boundary for the CURRENT engine (see the
    /// field doc). idempotent; cleared by [`OrderedNode::cutover`].
    pub fn set_view_ceiling(&mut self, ceiling: u64) {
        self.view_ceiling = Some(ceiling);
    }

    /// the last ENGINE-relative finalized view this node drained — the number
    /// the valset orchestrator observes. `None` since the last cutover.
    pub fn last_engine_view(&self) -> Option<u64> {
        self.last_engine_view
    }

    /// SUBMIT — propose a locally-originated msg into the agreed order. framed
    /// with `(origin, seq)` for a tie-free order key + replay identity. does NOT
    /// touch the local host: `app_hash()` is unchanged until the order delivers
    /// this frame back through [`OrderedNode::drain_delivered`] (the semantic
    /// shift — no optimistic echo).
    pub async fn submit(&mut self, signer: &PrivateKey, seq: u64, msg: Msg) -> Result<(), Error> {
        self.orderer.submit(encode_frame(signer, seq, &msg)).await
    }

    /// DRAIN — apply every frame the order delivered, STRICTLY in agreed order,
    /// via `host.submit`. returns the count applied (0 when idle) so a test can
    /// drive to a fixpoint deterministically.
    ///
    /// ## rejected vs fatal
    ///
    /// a DETERMINISTIC rejection (decode failure, module error, blown budget) is
    /// a no-op: every honest validator finalized the identical op and rejects it
    /// identically — the drain keeps going. a FATAL boundary fault
    /// ([`host::SubmitError::Fatal`]) is node-local: this registry is now
    /// indeterminate, so the drain STOPS and surfaces [`Error::Fatal`] — applying
    /// even one more finalized op would compound a state no validator agreed on.
    pub async fn drain_delivered(&mut self) -> Result<usize, Error> {
        let delivered = self.orderer.poll_delivered();
        let mut applied = 0usize;
        let mut last_view: Option<u64> = None;
        for (view, frame) in delivered {
            // a FINALIZED op counts as processed whether or not it applies
            // cleanly — and its VIEW advances the engine clock either way (the
            // view was agreed; discarding or rejecting its op is the same
            // deterministic no-op on every honest node). without this, a node
            // could never OBSERVE the views that carry it past its own cutover.
            applied += 1;
            last_view = Some(view);
            // the CUTOVER CEILING: frames finalized at or past the agreed
            // cutover view are DISCARDED — the same view-based rule on every
            // honest node, so a straggler finalizing during teardown on only
            // some nodes cannot fork app state.
            if let Some(ceiling) = self.view_ceiling {
                if view >= ceiling {
                    continue;
                }
            }
            // one that fails to decode, or that a module rejects, is a DETERMINISTIC
            // no-op: every honest validator finalized the identical op and handles it
            // identically (host-lent rolls back a rejected block, root unchanged), so
            // the chain cannot fork — AND a byzantine proposer cannot HALT honest nodes
            // by getting a malformed op finalized. (the `?`-propagate that used to be
            // here stalled the whole drain on one bad op — the liveness gap.)
            let Ok((origin, msg)) = decode_frame(&frame) else { continue };
            // the agreed view is the block coordinate: the APP HEIGHT is the
            // engine view offset by the epoch base, so heights and the logical
            // clock stay monotone across epoch cutovers — identical on every
            // validator. the frame carries the op's real submitter.
            let height = self.view_base + view;
            let ctx = BlockContext { height, consensus_time: height, origin };
            // surface each finalized block's effects for the reactor's worker
            // driver. a rejected op yields no outcome (deterministic no-op) and so
            // contributes no effects — same on every validator.
            match self.host.submit_at(ctx, msg).await {
                Ok(outcome) => self.effects.extend(outcome.effects),
                Err(host::SubmitError::Rejected(_)) => {}
                Err(e @ host::SubmitError::Fatal(_)) => return Err(e.into()),
            }
        }
        if let Some(view) = last_view {
            self.last_engine_view = Some(view);
            self.finalized = Some(host::FinalizedBlock {
                height: self.view_base + view,
                app_hash: self.host.app_hash(),
            });
        }
        Ok(applied)
    }

    /// the latest APPLIED consensus boundary — what a state-sync service serves
    /// from. `None` until the first delivered frame applies.
    pub fn finalized(&self) -> Option<host::FinalizedBlock> {
        self.finalized
    }

    /// the current app-hash of the wrapped host.
    pub fn app_hash(&self) -> StateRoot {
        self.host.app_hash()
    }

    /// take the effects accumulated by applied blocks since the last call. the
    /// host-owned reactor drains these, runs the assigned worker on each
    /// `WorkerRequest`, and submits the resulting `OracleResult` op back through
    /// the ordered lane (the oracle-as-op over consensus).
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    /// borrow the wrapped host (queries, module_root inspection, ...).
    pub fn host(&self) -> &Host {
        &self.host
    }
}
