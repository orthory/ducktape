//! The five FIXED engine lanes.
//!
//! A node registers vote / certificate / resolver / payload / fetch ONCE, on
//! five constant channels, and every engine this process ever spawns runs over
//! those same five. What used to make that impossible was engine respawn: a
//! cutover tears one engine down and stands the next one up, and a respawned
//! engine must never read its predecessor's messages — so each epoch was given
//! its own five channels out of a pre-registered bank.
//!
//! Two things replace the bank. Every frame we put on an engine lane is
//! `epoch: u64 (big-endian) ‖ simplex bytes`, and a per-lane DEMUX task owns
//! the registered receiver for the life of the process: it peels the prefix and
//! forwards the body to whichever consumer is currently installed — a seated
//! engine's mailbox, a parked replica's drain, or nothing at all. Retargeting
//! that one pointer is what a respawn does, and dropping the old mailbox is
//! what ends the old engine's read.
//!
//! The demux is not an optimization, it is the safety property. Simplex's own
//! batcher refuses a foreign-epoch message by BLOCKING the sender
//! (`commonware_p2p::block!(.., "epoch mismatch")`), so a lagging peer's
//! old-epoch vote must never reach the engine: dropping it here is what keeps
//! that peer's connection — the very link it needs to catch up — alive.
//!
//! The fetch lane is the one lane that takes any epoch ([`LaneEpoch::Any`]):
//! it is content-addressed payload catch-up, a lagging peer's fetch is still a
//! valid fetch, and its actor rejects a foreign epoch without blocking anyone.
//! It carries the same prefix as the other four so one sender type serves all
//! five channels.

use std::sync::{Arc, Mutex};

use commonware_cryptography::ed25519;
use commonware_p2p::authenticated::lookup::Network;
use commonware_p2p::{Message, Recipients};
use commonware_runtime::{IoBuf, IoBufs, Quota, Spawner, Supervisor};
use futures::StreamExt as _;

use crate::constants::{ENGINE_LANE_MAILBOX, MESH_QUOTA_BURST};
use crate::validator::{MeshSender, OverlayCtx};

/// the width of the big-endian epoch prefix every engine-lane frame carries.
const EPOCH_PREFIX: usize = std::mem::size_of::<u64>();

/// which epochs a lane's currently-installed consumer takes.
#[derive(Clone, Copy)]
pub(crate) enum LaneEpoch {
    /// every frame, whatever epoch it claims: the parked replica's certificate
    /// bridge and payload store (the fold driver verifies each finalization
    /// against its own epoch's quorum downstream, and payload bytes are
    /// content-addressed), and the fetch lane on a seated engine.
    Any,
    /// exactly this epoch: a seated engine's vote / certificate / resolver /
    /// payload lanes. Handing simplex a frame from another epoch makes its
    /// batcher BLOCK the peer that sent it.
    Only(u64),
}

impl LaneEpoch {
    fn takes(self, frame_epoch: u64) -> bool {
        match self {
            Self::Any => true,
            Self::Only(epoch) => epoch == frame_epoch,
        }
    }
}

/// where one lane's demux currently delivers. `None` — the lane of a role that
/// consumes it (a validator between `network.start()` and its first engine, a
/// parked replica's vote/resolver/fetch lanes, every lane of a sync-only
/// observer) — drops every frame. That IS the black-hole the bank spawned a
/// task per epoch for.
type LaneTarget = Option<LaneDelivery>;

struct LaneDelivery {
    accepts: LaneEpoch,
    tx: futures::channel::mpsc::Sender<Message<ed25519::PublicKey>>,
    /// frames shed since this target was installed, foreign-epoch and
    /// mailbox-full together. Reported once, at the next retarget.
    shed: u64,
}

/// one fixed engine lane: the registered mesh channel, plus the demux task
/// that owns its receiver for the life of the process.
pub(crate) struct EpochLane {
    name: &'static str,
    sender: MeshSender,
    target: Arc<Mutex<LaneTarget>>,
}

impl EpochLane {
    /// register `channel` and start its demux. Nothing is delivered until a
    /// [`EpochLane::seat`] or [`EpochLane::drain`] installs a consumer.
    fn register(
        context: &commonware_runtime::tokio::Context,
        network: &mut Network<OverlayCtx, ed25519::PrivateKey>,
        name: &'static str,
        channel: u64,
        quota: Quota,
    ) -> Self {
        let (sender, mut receiver) = network.register(channel, quota);
        let target: Arc<Mutex<LaneTarget>> = Arc::new(Mutex::new(None));
        let demux = Arc::clone(&target);
        context.child(name).spawn(move |_ctx| async move {
            use commonware_p2p::Receiver as _;
            loop {
                // the ONLY await in this loop: `deliver` takes the target lock
                // after it and releases it before the next pass, so the lock is
                // never held across an await.
                let Ok((peer, frame)) = receiver.recv().await else {
                    return;
                };
                deliver(name, &demux, peer, frame);
            }
        });
        Self {
            name,
            sender,
            target,
        }
    }

    /// install a fresh mailbox as this lane's delivery target and hand its
    /// receive half back. The PREVIOUS target's send half is dropped here —
    /// that is what ends a torn-down engine's (or a parked drain's) read of
    /// this lane, so no two consumers ever share it.
    fn install(
        &self,
        accepts: LaneEpoch,
    ) -> futures::channel::mpsc::Receiver<Message<ed25519::PublicKey>> {
        install(self.name, &self.target, accepts)
    }

    /// hand epoch `epoch`'s engine this lane: a sender that stamps `epoch` on
    /// every frame, and a receiver fed only what `accepts` takes.
    fn seat(&self, epoch: u64, accepts: LaneEpoch) -> (EpochSender, LaneMailbox) {
        let rx = self.install(accepts);
        (
            EpochSender {
                inner: self.sender.clone(),
                epoch,
            },
            LaneMailbox { rx },
        )
    }

    /// consume every frame this lane receives, at any epoch, through
    /// `on_frame` — the parked replica's certificate bridge and payload store.
    /// A later [`EngineLanes::seat`] replaces the target, which drops the
    /// mailbox and ends this task.
    pub(crate) fn drain(
        &self,
        context: &commonware_runtime::tokio::Context,
        mut on_frame: impl FnMut(Vec<u8>) + Send + 'static,
    ) {
        let mut rx = self.install(LaneEpoch::Any);
        context.child(self.name).spawn(move |_ctx| async move {
            while let Some((_peer, frame)) = rx.next().await {
                on_frame(frame.into());
            }
        });
    }
}

/// the five fixed engine lanes, registered once per role before
/// `network.start()` — the whole channel budget consensus costs this process.
pub(crate) struct EngineLanes {
    pub(crate) vote: EpochLane,
    pub(crate) certificate: EpochLane,
    pub(crate) resolver: EpochLane,
    pub(crate) payload: EpochLane,
    pub(crate) fetch: EpochLane,
}

impl EngineLanes {
    /// register all five. Every role does this — a validator, a parked
    /// replica, and a sync-only observer alike — because a message on an
    /// unregistered channel is a protocol violation that kills the sender's
    /// connection, and validators gossip to every tracked peer, not only to
    /// fellow participants.
    pub(crate) fn register(
        context: &commonware_runtime::tokio::Context,
        network: &mut Network<OverlayCtx, ed25519::PrivateKey>,
        quota: Quota,
    ) -> Self {
        use crate::constants::{
            CHANNEL_ENGINE_CERTIFICATE, CHANNEL_ENGINE_FETCH, CHANNEL_ENGINE_PAYLOAD,
            CHANNEL_ENGINE_RESOLVER, CHANNEL_ENGINE_VOTE,
        };
        Self {
            vote: EpochLane::register(context, network, "lane_vote", CHANNEL_ENGINE_VOTE, quota),
            certificate: EpochLane::register(
                context,
                network,
                "lane_certificate",
                CHANNEL_ENGINE_CERTIFICATE,
                quota,
            ),
            resolver: EpochLane::register(
                context,
                network,
                "lane_resolver",
                CHANNEL_ENGINE_RESOLVER,
                quota,
            ),
            payload: EpochLane::register(
                context,
                network,
                "lane_payload",
                CHANNEL_ENGINE_PAYLOAD,
                quota,
            ),
            fetch: EpochLane::register(context, network, "lane_fetch", CHANNEL_ENGINE_FETCH, quota),
        }
    }

    /// retarget all five at epoch `epoch`'s engine. The four consensus lanes
    /// take that epoch alone; fetch takes any (see the module note).
    pub(crate) fn seat(&self, epoch: u64) -> SeatedLanes {
        let only = LaneEpoch::Only(epoch);
        SeatedLanes {
            vote: self.vote.seat(epoch, only),
            certificate: self.certificate.seat(epoch, only),
            resolver: self.resolver.seat(epoch, only),
            payload: self.payload.seat(epoch, only),
            fetch: self.fetch.seat(epoch, LaneEpoch::Any),
        }
    }
}

/// one engine's five channel pairs, in `EngineLanes` order.
pub(crate) struct SeatedLanes {
    pub(crate) vote: (EpochSender, LaneMailbox),
    pub(crate) certificate: (EpochSender, LaneMailbox),
    pub(crate) resolver: (EpochSender, LaneMailbox),
    pub(crate) payload: (EpochSender, LaneMailbox),
    pub(crate) fetch: (EpochSender, LaneMailbox),
}

/// swap in a fresh mailbox as `target`'s delivery, returning its receive half
/// and dropping the previous send half (which is what ends the previous
/// consumer's read of the lane).
fn install(
    name: &'static str,
    target: &Mutex<LaneTarget>,
    accepts: LaneEpoch,
) -> futures::channel::mpsc::Receiver<Message<ed25519::PublicKey>> {
    let (tx, rx) = futures::channel::mpsc::channel(ENGINE_LANE_MAILBOX);
    let previous = target
        .lock()
        .expect("engine lane target")
        .replace(LaneDelivery {
            accepts,
            tx,
            shed: 0,
        });
    // ONE line per lane per epoch transition, carrying the whole run's shed
    // count — never one line per shed frame.
    tracing::debug!(
        target: "ducktape::consensus",
        lane = name,
        shed = previous.map(|p| p.shed).unwrap_or(0),
        "engine lane retargeted"
    );
    rx
}

/// route one received frame: peel its epoch, ask the installed target whether
/// it takes that epoch, hand the body over. Every rejection is a silent drop
/// counted on the target — the alternative, letting a foreign-epoch frame
/// reach simplex, gets the peer that sent it BLOCKED.
fn deliver(
    name: &'static str,
    target: &Mutex<LaneTarget>,
    peer: ed25519::PublicKey,
    mut frame: IoBuf,
) {
    let Some(frame_epoch) = peel_epoch(&mut frame) else {
        tracing::trace!(
            target: "ducktape::consensus",
            lane = name,
            reason = "short_frame",
            "engine lane frame dropped"
        );
        return;
    };
    let mut target = target.lock().expect("engine lane target");
    // no consumer installed: this role does not read this lane.
    let Some(delivery) = target.as_mut() else {
        return;
    };
    if !delivery.accepts.takes(frame_epoch) {
        delivery.shed += 1;
        tracing::trace!(
            target: "ducktape::consensus",
            lane = name,
            frame_epoch,
            reason = "foreign_epoch",
            "engine lane frame dropped"
        );
        return;
    }
    // drop-on-full, exactly the contract of the lane underneath: commonware
    // drops an inbound message when a channel's mailbox is full rather than
    // stalling the peer's connection.
    if delivery.tx.try_send((peer, frame)).is_err() {
        delivery.shed += 1;
        tracing::trace!(
            target: "ducktape::consensus",
            lane = name,
            frame_epoch,
            reason = "mailbox_full",
            "engine lane frame dropped"
        );
    }
}

/// take the epoch prefix off a received frame, leaving the simplex body.
/// `None` — the frame is shorter than its own prefix, which is junk no
/// consumer can decode.
fn peel_epoch(frame: &mut IoBuf) -> Option<u64> {
    if frame.len() < EPOCH_PREFIX {
        return None;
    }
    let prefix = frame.split_to(EPOCH_PREFIX);
    let bytes: [u8; EPOCH_PREFIX] = prefix.as_ref().try_into().expect("split_to yields 8 bytes");
    Some(u64::from_be_bytes(bytes))
}

/// the engine-side send half: the lane's raw mesh sender, stamping this
/// engine's epoch onto every frame so a peer's demux can route it.
#[derive(Clone)]
pub(crate) struct EpochSender {
    inner: MeshSender,
    epoch: u64,
}

impl commonware_p2p::LimitedSender for EpochSender {
    type PublicKey = ed25519::PublicKey;
    type Checked<'a>
        = EpochChecked<'a>
    where
        Self: 'a;

    fn check(
        &mut self,
        recipients: Recipients<Self::PublicKey>,
    ) -> Result<Self::Checked<'_>, std::time::SystemTime> {
        let inner = commonware_p2p::LimitedSender::check(&mut self.inner, recipients)?;
        Ok(EpochChecked {
            inner,
            epoch: self.epoch,
        })
    }
}

pub(crate) struct EpochChecked<'a> {
    inner: <MeshSender as commonware_p2p::LimitedSender>::Checked<'a>,
    epoch: u64,
}

impl commonware_p2p::CheckedSender for EpochChecked<'_> {
    type PublicKey = ed25519::PublicKey;

    fn recipients(&self) -> Vec<Self::PublicKey> {
        commonware_p2p::CheckedSender::recipients(&self.inner)
    }

    fn send(
        self,
        message: impl Into<IoBufs> + Send,
        priority: bool,
    ) -> commonware_actor::Unreliable<commonware_actor::Feedback> {
        let mut framed: IoBufs = message.into();
        // prepend, never copy: `IoBufs` is a chain, so the prefix rides as its
        // own 8-byte chunk in front of the simplex bytes.
        framed.prepend(IoBuf::copy_from_slice(&self.epoch.to_be_bytes()));
        commonware_p2p::CheckedSender::send(self.inner, framed, priority)
    }
}

/// the engine-side receive half: what the demux hands the engine that is
/// currently seated on the lane.
pub(crate) struct LaneMailbox {
    rx: futures::channel::mpsc::Receiver<Message<ed25519::PublicKey>>,
}

impl std::fmt::Debug for LaneMailbox {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LaneMailbox").finish_non_exhaustive()
    }
}

/// the mailbox's send half was dropped: this engine has been retargeted away
/// from its lane (a cutover), or the mesh is shutting down.
#[derive(Debug)]
pub(crate) struct LaneRetargeted;

impl std::fmt::Display for LaneRetargeted {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("engine lane retargeted")
    }
}

impl std::error::Error for LaneRetargeted {}

impl commonware_p2p::Receiver for LaneMailbox {
    type Error = LaneRetargeted;
    type PublicKey = ed25519::PublicKey;

    async fn recv(&mut self) -> Result<Message<Self::PublicKey>, Self::Error> {
        self.rx.next().await.ok_or(LaneRetargeted)
    }
}

const _: () = assert!(
    ENGINE_LANE_MAILBOX >= MESH_QUOTA_BURST,
    "an engine mailbox must absorb at least one peer's burst"
);

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::Signer as _;

    const LANE: &str = "lane_test";

    fn peer() -> ed25519::PublicKey {
        ed25519::PrivateKey::from_seed(7).public_key()
    }

    fn framed(epoch: u64, body: &[u8]) -> IoBuf {
        let mut frame = epoch.to_be_bytes().to_vec();
        frame.extend_from_slice(body);
        IoBuf::copy_from_slice(&frame)
    }

    /// what the demux left in a mailbox, bodies only.
    fn taken(
        rx: &mut futures::channel::mpsc::Receiver<Message<ed25519::PublicKey>>,
    ) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        while let Ok((_peer, body)) = rx.try_recv() {
            out.push(body.as_ref().to_vec());
        }
        out
    }

    #[test]
    fn a_seated_engine_takes_its_own_epoch_and_drops_every_other() {
        let target = Mutex::new(LaneTarget::None);
        let mut rx = install(LANE, &target, LaneEpoch::Only(7));
        deliver(LANE, &target, peer(), framed(6, b"stale"));
        deliver(LANE, &target, peer(), framed(7, b"mine"));
        deliver(LANE, &target, peer(), framed(8, b"ahead"));
        // the body arrives WITHOUT the prefix — simplex decodes its own bytes.
        assert_eq!(taken(&mut rx), vec![b"mine".to_vec()]);
    }

    #[test]
    fn the_fetch_lane_takes_every_epoch() {
        let target = Mutex::new(LaneTarget::None);
        let mut rx = install(LANE, &target, LaneEpoch::Any);
        deliver(LANE, &target, peer(), framed(0, b"a"));
        deliver(LANE, &target, peer(), framed(u64::MAX, b"b"));
        assert_eq!(taken(&mut rx), vec![b"a".to_vec(), b"b".to_vec()]);
    }

    #[test]
    fn a_retarget_moves_delivery_and_closes_the_old_mailbox() {
        let target = Mutex::new(LaneTarget::None);
        let mut old = install(LANE, &target, LaneEpoch::Only(1));
        deliver(LANE, &target, peer(), framed(1, b"epoch one"));

        let mut new = install(LANE, &target, LaneEpoch::Only(2));
        deliver(LANE, &target, peer(), framed(1, b"lagging peer"));
        deliver(LANE, &target, peer(), framed(2, b"epoch two"));

        // what landed before the retarget is still readable; nothing new is.
        assert_eq!(taken(&mut old), vec![b"epoch one".to_vec()]);
        let closed = matches!(
            old.try_recv(),
            Err(futures::channel::mpsc::TryRecvError::Closed)
        );
        assert!(closed, "the old mailbox is closed, not merely empty");
        assert_eq!(taken(&mut new), vec![b"epoch two".to_vec()]);
    }

    #[test]
    fn a_frame_shorter_than_its_prefix_is_dropped_not_a_panic() {
        let target = Mutex::new(LaneTarget::None);
        let mut rx = install(LANE, &target, LaneEpoch::Any);
        for len in 0..EPOCH_PREFIX {
            deliver(
                LANE,
                &target,
                peer(),
                IoBuf::copy_from_slice(&vec![0u8; len]),
            );
        }
        assert!(taken(&mut rx).is_empty());
        // exactly the prefix and nothing else is well-formed: an empty body.
        deliver(LANE, &target, peer(), framed(3, b""));
        assert_eq!(taken(&mut rx), vec![Vec::<u8>::new()]);
    }

    #[test]
    fn an_uninstalled_lane_black_holes() {
        let target = Mutex::new(LaneTarget::None);
        deliver(LANE, &target, peer(), framed(0, b"nobody reads this"));
        // no consumer, no panic, no growth: the assertion is that a later
        // install starts clean.
        let mut rx = install(LANE, &target, LaneEpoch::Any);
        assert!(taken(&mut rx).is_empty());
    }
}
