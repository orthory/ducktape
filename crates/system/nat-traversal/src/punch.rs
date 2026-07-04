use std::net::SocketAddr;

use crate::{Coordinator, Msg, NodeKey, Side, relay::RelaySplice, simnat::SimNat};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PunchPlan {
    pub local_mapped: SocketAddr,
    pub peer_reflexive: SocketAddr,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum PunchError {
    #[error("coordinator gave no reflexive for peer")]
    NoReflexive,
    #[error("hole-punch did not open a bidirectional path")]
    NotReachable,
}

// A fixed coordinator address the SimNat sends toward during discovery.
fn coord_addr() -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 3478)
}

// A fresh pair always fails delivery on round one: the two `SimNat`s are
// independent state machines with no shared clock, and `punch_once` sends
// A's datagram strictly before B's, so B has not yet opened its own filter
// toward A when A's datagram would arrive. Bound the retry so a genuinely
// unreachable pair still terminates instead of looping forever.
const MAX_PUNCH_ATTEMPTS: u32 = 4;

// One side's rendezvous-resolved coordinates going into a punch attempt.
#[derive(Clone, Copy)]
struct PunchSide {
    key: NodeKey,
    mapped: SocketAddr,
    peer: SocketAddr,
}

/// Attempt exactly one round of simultaneous open: each side sends a single
/// Punch toward the other's reflexive address, in a fixed order (`a` then
/// `b`). Returns, for each side, whether *this round's* datagram was
/// actually admitted by the peer's NAT filter — not the aggregate final
/// state, which is a much weaker property that final-state-only checks
/// conflate with delivery. Because of the fixed send order, the first
/// element is guaranteed `false` on a fresh pair's first round: this is
/// precisely why a single one-shot attempt with no retry is not sufficient
/// for simultaneous open.
fn punch_once(a: PunchSide, b: PunchSide, a_nat: &mut SimNat, b_nat: &mut SimNat) -> (bool, bool) {
    let _ = a_nat.send(internal(&a.key), a.peer);
    let a_delivered = b_nat.allow_inbound(b.mapped, a.mapped);
    let _ = b_nat.send(internal(&b.key), b.peer);
    let b_delivered = a_nat.allow_inbound(a.mapped, b.mapped);
    (a_delivered, b_delivered)
}

/// Drive simultaneous-open with retry until BOTH directions have had a datagram
/// actually admitted (observed per-round, not inferred from final filter
/// state), or the attempt budget is exhausted. Shared by `drive_simulated` and
/// `drive_rebind_reconnect`.
fn punch_until_bidirectional(
    a_side: PunchSide,
    b_side: PunchSide,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
) -> Result<(), PunchError> {
    let mut a_delivered = false;
    let mut b_delivered = false;
    for _ in 0..MAX_PUNCH_ATTEMPTS {
        if a_delivered && b_delivered {
            break;
        }
        let (a_ok, b_ok) = punch_once(a_side, b_side, a_nat, b_nat);
        a_delivered |= a_ok;
        b_delivered |= b_ok;
    }
    if !a_delivered || !b_delivered {
        return Err(PunchError::NotReachable);
    }
    Ok(())
}

/// Deterministic in-memory choreography of the full discover→rendezvous→punch
/// dance for two endpoints behind their own `SimNat`. No real sockets: this is
/// the CI proof that simultaneous-open works for the restricted-cone case.
pub fn drive_simulated(
    a_key: NodeKey,
    b_key: NodeKey,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
    coord: &mut Coordinator,
) -> Result<(PunchPlan, PunchPlan), PunchError> {
    // 1. Each node registers: the datagram traverses its NAT (opening a hole to
    //    the coordinator) and the coordinator records the observed mapped addr.
    let a_mapped = a_nat.send(internal(&a_key), coord_addr());
    let b_mapped = b_nat.send(internal(&b_key), coord_addr());
    coord.handle(a_mapped, Msg::Register { key: a_key });
    coord.handle(b_mapped, Msg::Register { key: b_key });

    // 2. A looks up B; the coordinator returns B's reflexive and issues
    //    PunchSync to both mapped addresses.
    let out = coord.handle(a_mapped, Msg::Lookup { key: b_key });
    let mut a_peer = None;
    let mut b_peer = None;
    for (dst, msg) in out {
        if let Msg::PunchSync { peer_reflexive, .. } = msg {
            if dst == a_mapped {
                a_peer = Some(peer_reflexive);
            } else if dst == b_mapped {
                b_peer = Some(peer_reflexive);
            }
        }
    }
    let a_peer = a_peer.ok_or(PunchError::NoReflexive)?;
    let b_peer = b_peer.ok_or(PunchError::NoReflexive)?;

    // 3. Simultaneous open, with retry. A single one-shot packet from each
    //    side is not enough to prove bidirectional delivery: checking only
    //    the state *after* both sends have run always looks symmetric,
    //    regardless of whether either individual datagram actually arrived
    //    when it was sent (see `punch_once`'s doc comment and the
    //    `a_single_one_shot_attempt_drops_as_first_packet` regression test
    //    below — Slice 0a review). Retry each side's punch until a round
    //    actually delivers it, observed per-round, not from final filter state.
    let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
    let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };
    punch_until_bidirectional(a_side, b_side, a_nat, b_nat)?;

    Ok((
        PunchPlan { local_mapped: a_mapped, peer_reflexive: a_peer },
        PunchPlan { local_mapped: b_mapped, peer_reflexive: b_peer },
    ))
}

/// The proof a rebind reconnect produces: the reflexive before and after the
/// rebind (they must differ), plus the fresh punch plans on the new mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebindProof {
    pub old_a_reflexive: SocketAddr,
    pub new_a_reflexive: SocketAddr,
    pub a_plan: PunchPlan,
    pub b_plan: PunchPlan,
}

/// Deterministic full rebinding path: punch once, then A's NAT rebinds, A
/// re-runs STUN and re-advertises under a HIGHER monotonic nonce (superseding
/// its stale mapping), B re-resolves the new reflexive via the coordinator, and
/// the pair reconnects on the new mapping. This is the CI proof for
/// "endpoint-churn re-advertisement" (Acceptance §1) and the design's
/// "NAT rebinding → re-run STUN and re-advertise under a higher monotonic
/// nonce" fallback rule.
pub fn drive_rebind_reconnect(
    a_key: NodeKey,
    b_key: NodeKey,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
    coord: &mut Coordinator,
) -> Result<RebindProof, PunchError> {
    // 1. Establish the initial direct path (also registers both nodes).
    let (a_plan0, _b_plan0) = drive_simulated(a_key, b_key, a_nat, b_nat, coord)?;
    let old_a_reflexive = a_plan0.local_mapped;

    // 2. A's NAT rebinds: its mappings + holes are dropped.
    a_nat.rebind();

    // 3. A re-runs STUN: the datagram traverses the rebound NAT to a FRESH
    //    reflexive, which the coordinator observes.
    let new_a_mapped = a_nat.send(internal(&a_key), coord_addr());

    // 4. A re-advertises under a strictly-higher nonce; it must supersede.
    if coord.readvertise(a_key, new_a_mapped, 1) != crate::AdvertOutcome::Superseded {
        return Err(PunchError::NoReflexive);
    }

    // 5. B re-resolves: its Lookup now returns A's NEW reflexive, and the
    //    coordinator fans out PunchSync to both the new A mapping and B.
    let b_mapped = a_plan0_b_mapped(b_key, b_nat);
    let out = coord.handle(b_mapped, Msg::Lookup { key: a_key });
    let mut b_peer = None; // A's new reflexive, as B sees it
    let mut a_peer = None; // B's reflexive, as A sees it (via the fan-out)
    for (dst, msg) in out {
        if let Msg::PunchSync { peer_reflexive, .. } = msg {
            if dst == b_mapped {
                b_peer = Some(peer_reflexive);
            } else if dst == new_a_mapped {
                a_peer = Some(peer_reflexive);
            }
        }
    }
    let b_peer = b_peer.ok_or(PunchError::NoReflexive)?;
    let a_peer = a_peer.ok_or(PunchError::NoReflexive)?;

    // 6. Reconnect on the new mapping.
    let a_side = PunchSide { key: a_key, mapped: new_a_mapped, peer: a_peer };
    let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };
    punch_until_bidirectional(a_side, b_side, a_nat, b_nat)?;

    Ok(RebindProof {
        old_a_reflexive,
        new_a_reflexive: new_a_mapped,
        a_plan: PunchPlan { local_mapped: new_a_mapped, peer_reflexive: a_peer },
        b_plan: PunchPlan { local_mapped: b_mapped, peer_reflexive: b_peer },
    })
}

// B's coordinator-facing mapping is stable (B did not rebind), so re-deriving it
// is idempotent and matches what registration observed.
fn a_plan0_b_mapped(b_key: NodeKey, b_nat: &mut SimNat) -> SocketAddr {
    b_nat.send(internal(&b_key), coord_addr())
}

// Deterministic internal socket for a node key in the simulation.
fn internal(key: &NodeKey) -> SocketAddr {
    use std::net::{IpAddr, Ipv4Addr};
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 168, 0, key.0[0])), 51820)
}

/// The proof a relay fallback produces: the relay endpoint each side must point
/// its WireGuard peer at (`apply_tunnel_plan`'s `peer_endpoint_override` on the
/// fallback path), plus the opaque bytes actually delivered end to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayFallbackProof {
    pub a_relay_endpoint: SocketAddr,
    pub b_relay_endpoint: SocketAddr,
    pub delivered_to_b: Vec<u8>,
    pub delivered_to_a: Vec<u8>,
}

/// Outcome of the reachability dance: a direct hole-punched path, or — only
/// when hole-punch failed — the coordinator relay fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FallbackOutcome {
    Punched { a: PunchPlan, b: PunchPlan },
    Relayed(RelayFallbackProof),
}

// The two relay egress ports the coordinator would bind for a session in the
// deterministic model. The real coordinator binds ephemeral ports and reports
// the actual addresses in the `RelayGrant` (Task 6); here they are derived
// from the session id so the model stays reproducible.
fn relay_side_addrs(session: u64) -> (SocketAddr, SocketAddr) {
    use std::net::{IpAddr, Ipv4Addr};
    let ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    let base = 4000u16.wrapping_add((session as u16).wrapping_mul(2));
    (
        SocketAddr::new(ip, base),
        SocketAddr::new(ip, base.wrapping_add(1)),
    )
}

/// Attempt hole-punch FIRST; only on `PunchError::NotReachable` fall back to
/// the coordinator ciphertext relay and prove OPAQUE bidirectional delivery
/// through both NATs. This is the CI proof that a symmetric-NAT pair still
/// reaches each other with neither exposing an inbound port.
#[allow(clippy::too_many_arguments)]
pub fn drive_with_relay_fallback(
    a_key: NodeKey,
    b_key: NodeKey,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
    coord: &mut Coordinator,
    a_payload: &[u8],
    b_payload: &[u8],
) -> Result<FallbackOutcome, PunchError> {
    // Hole-punch first. `drive_simulated` registers both nodes with the
    // coordinator as a side effect, which is exactly what relay allocation
    // needs (source->key binding).
    match drive_simulated(a_key, b_key, a_nat, b_nat, coord) {
        Ok((a, b)) => return Ok(FallbackOutcome::Punched { a, b }),
        Err(PunchError::NotReachable) => {}
        Err(e) => return Err(e),
    }

    // The coordinator-facing mapping is idempotent (cone: stable; symmetric:
    // same (internal, coord) key), so re-deriving each mapped source is safe
    // and matches what registration observed.
    let a_mapped = a_nat.send(internal(&a_key), coord_addr());
    let b_mapped = b_nat.send(internal(&b_key), coord_addr());

    // Control plane: both sides request the relay for the same unordered pair.
    let (session, _a_side) = coord
        .request_relay(a_mapped, b_key, 0)
        .ok_or(PunchError::NoReflexive)?;
    let (session_b, _b_side) = coord
        .request_relay(b_mapped, a_key, 0)
        .ok_or(PunchError::NoReflexive)?;
    debug_assert_eq!(session, session_b, "one session per unordered pair");
    let (a_relay_endpoint, b_relay_endpoint) = relay_side_addrs(session);

    // Data plane: each side sends its opaque payload OUT to its (fixed) relay
    // endpoint. Because the destination is stable, even a symmetric NAT opens a
    // durable hole toward the relay, so return traffic from the relay is
    // admitted — this is why the relay beats symmetric NAT.
    let a_mapped_relay = a_nat.send(internal(&a_key), a_relay_endpoint);
    let b_mapped_relay = b_nat.send(internal(&b_key), b_relay_endpoint);

    let mut splice = RelaySplice::new(a_relay_endpoint, b_relay_endpoint, 0);
    // A sends first: B's source not yet learned, so it is buffered/dropped.
    let _ = splice.ingress(Side::A, a_mapped_relay, 1, a_payload.to_vec());
    // B sends: A's source known -> forward B's payload toward A via a_egress.
    let to_a = splice
        .ingress(Side::B, b_mapped_relay, 2, b_payload.to_vec())
        .ok_or(PunchError::NotReachable)?;
    // A re-sends (real WireGuard retransmits): now forward A's payload to B.
    let to_b = splice
        .ingress(Side::A, a_mapped_relay, 3, a_payload.to_vec())
        .ok_or(PunchError::NotReachable)?;

    // Assert ACTUAL delivery: each NAT must admit the relay's egress datagram.
    if !b_nat.allow_inbound(b_mapped_relay, to_b.from)
        || !a_nat.allow_inbound(a_mapped_relay, to_a.from)
    {
        return Err(PunchError::NotReachable);
    }

    Ok(FallbackOutcome::Relayed(RelayFallbackProof {
        a_relay_endpoint,
        b_relay_endpoint,
        delivered_to_b: to_b.payload,
        delivered_to_a: to_a.payload,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Coordinator, NodeKey, simnat::SimNat};
    use std::net::{IpAddr, Ipv4Addr};

    // Resolve the (a_peer, b_peer) reflexive addresses via the same
    // register+lookup choreography `drive_simulated` uses, without also
    // performing the punch step — lets tests exercise the punch step in
    // isolation.
    fn rendezvous(
        a_key: NodeKey,
        b_key: NodeKey,
        a_nat: &mut SimNat,
        b_nat: &mut SimNat,
        coord: &mut Coordinator,
    ) -> (SocketAddr, SocketAddr, SocketAddr, SocketAddr) {
        let a_mapped = a_nat.send(internal(&a_key), coord_addr());
        let b_mapped = b_nat.send(internal(&b_key), coord_addr());
        coord.handle(a_mapped, Msg::Register { key: a_key });
        coord.handle(b_mapped, Msg::Register { key: b_key });

        let out = coord.handle(a_mapped, Msg::Lookup { key: b_key });
        let mut a_peer = None;
        let mut b_peer = None;
        for (dst, msg) in out {
            if let Msg::PunchSync { peer_reflexive, .. } = msg {
                if dst == a_mapped {
                    a_peer = Some(peer_reflexive);
                } else if dst == b_mapped {
                    b_peer = Some(peer_reflexive);
                }
            }
        }
        (a_mapped, b_mapped, a_peer.unwrap(), b_peer.unwrap())
    }

    #[test]
    fn a_single_one_shot_attempt_drops_as_first_packet() {
        // Regression test for the gap where `drive_simulated` only checked
        // the *final* NAT-filter state after both sides had punched, never
        // whether a single, non-retried datagram from each side was
        // actually delivered under the real send order. `punch_once` sends
        // A's packet strictly before B's, so on a fresh pair B has not yet
        // opened its own filter toward A when A's datagram would arrive —
        // it is silently dropped, exactly as a real restricted-cone NAT
        // would drop it. A naive implementation with no retry would lose
        // this datagram forever even though a final-state check (both
        // filters end up open) would report success.
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let (a_mapped, b_mapped, a_peer, b_peer) =
            rendezvous(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord);
        let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
        let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };

        let (a_delivered, b_delivered) = punch_once(a_side, b_side, &mut a_nat, &mut b_nat);

        assert!(!a_delivered, "A's first punch must be dropped before B opens its filter");
        assert!(b_delivered, "B's punch lands because A already opened its filter this round");
    }

    #[test]
    fn rebind_then_reresolve_then_reconnect_on_the_new_mapping() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let proof = drive_rebind_reconnect(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord)
            .expect("rebind reconnect");

        // The reflexive actually MOVED, and B re-resolved the NEW one.
        assert_ne!(proof.old_a_reflexive, proof.new_a_reflexive);
        assert_eq!(proof.a_plan.local_mapped, proof.new_a_reflexive);
        assert_eq!(
            proof.b_plan.peer_reflexive, proof.new_a_reflexive,
            "B reconnected against A's superseding reflexive, not the stale one"
        );
        // Bidirectional reachability re-established on the new mapping.
        assert!(a_nat.allow_inbound(proof.a_plan.local_mapped, proof.b_plan.local_mapped));
        assert!(b_nat.allow_inbound(proof.b_plan.local_mapped, proof.a_plan.local_mapped));
    }

    #[test]
    fn two_hidden_endpoints_punch_through_restricted_cone() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let (a_plan, b_plan) =
            drive_simulated(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord).expect("punch");

        // Each side ended up with the other's reflexive address, and each NAT
        // now admits the other's inbound datagrams: bidirectional reachability
        // with neither exposing an inbound port.
        assert_eq!(a_plan.peer_reflexive, b_plan.local_mapped);
        assert_eq!(b_plan.peer_reflexive, a_plan.local_mapped);
        assert!(a_nat.allow_inbound(a_plan.local_mapped, b_plan.local_mapped));
        assert!(b_nat.allow_inbound(b_plan.local_mapped, a_plan.local_mapped));
    }

    #[test]
    fn symmetric_nat_pair_fails_hole_punch_with_not_reachable() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let err = drive_simulated(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord)
            .expect_err("symmetric NAT must defeat hole-punch");
        assert_eq!(err, PunchError::NotReachable);
    }

    #[test]
    fn symmetric_pair_falls_back_to_relay_and_delivers_bidirectionally() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let outcome = drive_with_relay_fallback(
            a_key,
            b_key,
            &mut a_nat,
            &mut b_nat,
            &mut coord,
            b"ping-from-a",
            b"pong-from-b",
        )
        .expect("relay fallback");

        match outcome {
            FallbackOutcome::Relayed(p) => {
                // ACTUAL delivery of the opaque bytes, not just NAT-filter state.
                assert_eq!(p.delivered_to_b, b"ping-from-a");
                assert_eq!(p.delivered_to_a, b"pong-from-b");
                // Two distinct relay ports, one per side.
                assert_ne!(p.a_relay_endpoint, p.b_relay_endpoint);
            }
            FallbackOutcome::Punched { .. } => panic!("symmetric pair must NOT hole-punch"),
        }
    }

    #[test]
    fn cone_pair_punches_and_never_touches_the_relay() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let outcome = drive_with_relay_fallback(
            a_key, b_key, &mut a_nat, &mut b_nat, &mut coord, b"a", b"b",
        )
        .expect("punch");

        match outcome {
            FallbackOutcome::Punched { a, b } => {
                assert_eq!(a.peer_reflexive, b.local_mapped);
                assert_eq!(b.peer_reflexive, a.local_mapped);
            }
            FallbackOutcome::Relayed(_) => panic!("a punchable pair must not use the relay"),
        }
    }
}
