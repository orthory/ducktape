//! The machine's parked work: every operation whose outcome arrives as a
//! later [`Event`](crate::Event), as DATA — never a closure — so the
//! machine's whole state stays inspectable, replayable, and (in the arc's
//! later phases) snapshot-able.
//!
//! Two lifetimes coexist here. [`PendingOp`] entries wait on the HOST'S
//! runtime (a resolver op, the intro-ack datagram): commands interleave with
//! them, so every resumption re-validates against current state and a
//! `Retarget` clears them wholesale. [`WgCont`] waits only on the host's
//! SYNCHRONOUS interface push: the outcome is stepped back in before
//! anything else drains, so it is a singleton that never survives across
//! foreign events.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use commonware_cryptography::ed25519;
use wireguard::effect::PeerTunnelConfig;
use wireguard::{
    EndpointRecord, MeshVersion, MeshView, SignedEndpointRecord, ValidatorIdentity, X25519PublicKey,
};

use crate::contract::{CmdToken, MeshEpochEvent, ReqId};
use crate::epoch::Role;

/// One started host operation, keyed by its [`ReqId`], carrying exactly the
/// checkpoint its resumption needs. The variant families mirror the effect
/// that started them: `*Endpoint` waits on an advertised-endpoint resolve,
/// `*Rendezvous` on a by-identity rendezvous lookup, [`Self::IntroAck`] on
/// the invite bootstrap's awaited datagram.
pub(crate) enum PendingOp {
    /// The cold-restart restore resolving one remembered record's endpoint;
    /// joins into [`PendingRestore`].
    RestoreEndpoint {
        owner: ValidatorIdentity,
        advertised: SocketAddr,
    },
    /// The peers-locked-mesh adoption resolving one peer record's endpoint;
    /// joins into [`PendingAdopt`].
    AdoptEndpoint {
        peer: ValidatorIdentity,
        advertised: SocketAddr,
        wireguard_public_key: X25519PublicKey,
    },
    /// A post-lock member re-advertisement resolving its advertised
    /// endpoint before re-tunneling the member in place.
    ReadvertisedEndpoint {
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
        advertised: SocketAddr,
    },
    /// A standby's accepted record (member side) resolving its advertised
    /// endpoint before the pre-warm merge.
    StandbyPrewarmEndpoint(StandbyPrewarm),
    /// A member's record (standby side) resolving its advertised endpoint
    /// before the pre-warm merge.
    MemberPrewarmEndpoint {
        record: EndpointRecord,
        advertised: SocketAddr,
    },
    /// A handshake-time resolve for a peer with an advertised endpoint: a
    /// punched result records an override, written through to the live
    /// interface when it lands after the epoch applied.
    PeerEndpoint { peer: ValidatorIdentity },
    /// An endpoint-less re-advertisement's by-identity rendezvous fallback.
    ReadvertisedRendezvous {
        owner: ValidatorIdentity,
        signed: SignedEndpointRecord,
        via: ValidatorIdentity,
    },
    /// The standby sweep rendezvousing an endpoint-less pre-warm member.
    StandbyPrewarmRendezvous { peer: ValidatorIdentity },
    /// The by-identity rendezvous fallback for an endpoint-less member peer
    /// (handshake time and the member sweep share it).
    PeerRendezvous { peer: ValidatorIdentity },
    /// The coordinated-invite bootstrap rendezvousing its inviter.
    InviteRendezvous {
        token: CmdToken,
        peer: ed25519::PublicKey,
        wireguard_public_key: X25519PublicKey,
        intro: Vec<u8>,
    },
    /// The coordinated-invite bootstrap awaiting the inviter's intro ack.
    IntroAck { token: CmdToken },
}

/// The standby-record pre-warm continuation: everything the merge needs
/// once the record's advertised endpoint resolves.
pub(crate) struct StandbyPrewarm {
    pub(crate) owner: ValidatorIdentity,
    pub(crate) signed: SignedEndpointRecord,
    pub(crate) via: ValidatorIdentity,
    pub(crate) first_contact: bool,
    pub(crate) advertised: SocketAddr,
}

/// What to do when the single in-flight interface push resolves. Held in
/// [`super::Driver::wg`]; at most one exists because the host performs the
/// push synchronously and steps the outcome back before draining anything
/// else.
pub(crate) enum WgCont {
    /// The cold-restart restore's apply; success hands the restored standby
    /// records to the epoch tail, failure degrades to live assembly.
    Restore(RestoreApply),
    /// The epoch's ONE apply (validated plans becoming the new base).
    EpochApply {
        view: MeshView,
        base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
        plans_len: usize,
    },
    /// The peers-locked-mesh adoption's apply.
    Adopt {
        version: MeshVersion,
        records: Vec<EndpointRecord>,
        base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
        peer_count: usize,
    },
    /// A layered reconfiguration of the live interface (re-advertisement,
    /// pre-warm change, endpoint write-through); `follow` names the
    /// observation that lands only if the push does.
    Layers(LayersFollowUp),
    /// A join-window invite install; the outcome IS the command's reply.
    InviteInstall {
        token: CmdToken,
        peer: ed25519::PublicKey,
    },
    /// The coordinated-invite bootstrap's install; success proceeds to the
    /// awaited intro datagram, failure is the command's reply.
    InviteBootstrap {
        token: CmdToken,
        peer: ed25519::PublicKey,
        endpoint: SocketAddr,
        intro: Vec<u8>,
    },
}

/// The result-gated half of a layered push: what the machine observes once
/// the push actually lands. (Everything result-INDEPENDENT — floods, heals,
/// first-contact sends — is emitted alongside the push, never parked here.)
pub(crate) enum LayersFollowUp {
    /// A member's fresh life was re-tunneled in place.
    Readvertised { owner: ValidatorIdentity },
    /// A pre-warm change reached the interface.
    Prewarm,
    /// A late endpoint resolution was written through to the live
    /// interface.
    EndpointWriteThrough {
        peer: ValidatorIdentity,
        endpoint: SocketAddr,
    },
}

/// The cold-restart restore mid-resolution: the boot retarget parks here
/// while the remembered records' endpoints resolve, then joins into one
/// interface push. The epoch itself does not exist yet — it is built by the
/// retarget tail once the push settles.
pub(crate) struct PendingRestore {
    pub event: MeshEpochEvent,
    pub role: Role,
    pub mesh_epoch: u64,
    pub records: Vec<EndpointRecord>,
    pub standby_records: Vec<SignedEndpointRecord>,
    pub member_pk_of: BTreeMap<ValidatorIdentity, ed25519::PublicKey>,
    pub endpoints: BTreeMap<ValidatorIdentity, Option<SocketAddr>>,
    pub outstanding: BTreeSet<ReqId>,
}

/// The restore's apply in flight: everything its settlement needs.
pub(crate) struct RestoreApply {
    pub event: MeshEpochEvent,
    pub role: Role,
    pub base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    pub standby_records: Vec<SignedEndpointRecord>,
    pub mesh_epoch: u64,
    pub peer_count: usize,
}

/// The peers-locked-mesh adoption mid-resolution: endpoint resolves for the
/// peers' records join here into one apply. While this exists the epoch's
/// `advance` holds off — the adoption's own completion re-advances.
pub(crate) struct PendingAdopt {
    pub version: MeshVersion,
    pub records: Vec<EndpointRecord>,
    pub base: BTreeMap<ValidatorIdentity, PeerTunnelConfig>,
    pub outstanding: BTreeSet<ReqId>,
}
