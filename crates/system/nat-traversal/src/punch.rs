use std::net::SocketAddr;

use crate::{Coordinator, Msg, NodeKey, simnat::SimNat};

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

    // 4. A re-advertises under a strictly-higher nonce, over the SAME wire
    //    message + `handle` dispatch the real UDP coordinator loop uses
    //    (`Msg::Readvertise`), not the in-process `readvertise` shortcut — so the
    //    deterministic proof exercises the live protocol path. The coordinator
    //    re-observes `new_a_mapped` as the source and applies the nonce guard.
    //    Supersession is asserted end-to-end by step 5's re-resolution below (a
    //    stale mapping would resolve the OLD reflexive and fail `NoReflexive`).
    let _ = coord.handle(new_a_mapped, Msg::Readvertise { key: a_key, nonce: 1 });

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
    fn adverse_interleave_first_datagram_drops_but_retry_delivers_both_directions() {
        // The adverse case: A's punch is sent strictly BEFORE B has opened its
        // filter toward A (fixed send order in `punch_once`). A final-state-only
        // check would see both filters eventually open and wrongly call it a
        // success on round 1; the real proof observes each datagram AT SEND TIME.
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let (a_mapped, b_mapped, a_peer, b_peer) =
            rendezvous(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord);
        let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
        let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };

        // Round 1 (adverse): A's datagram is DROPPED (B's filter not yet open);
        // B's lands (A opened its filter first this round).
        let (a1, b1) = punch_once(a_side, b_side, &mut a_nat, &mut b_nat);
        assert!(!a1, "A's FIRST punch is dropped under the adverse order");
        assert!(b1, "B's punch is delivered because A already opened its filter");

        // Round 2 (retry): A's retransmit is now admitted — B opened its filter
        // in round 1. BOTH directions have now had a datagram actually delivered,
        // observed per-round, not inferred from final filter state.
        let (a2, _b2) = punch_once(a_side, b_side, &mut a_nat, &mut b_nat);
        assert!(a2, "A's retransmit is delivered on round 2: real bidirectional delivery");

        // And the full driver reaches the same success on the same fresh pair.
        let mut a_nat2 = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat2 = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord2 = Coordinator::new();
        drive_simulated(a_key, b_key, &mut a_nat2, &mut b_nat2, &mut coord2)
            .expect("driver delivers both directions despite the adverse first-drop");
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
        // With no relay fallback (the coordinator is rendezvous-only), a
        // symmetric-NAT pair terminally fails: `NotReachable` is the honest
        // outcome the reachability plane surfaces, not a degraded path.
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
    fn punched_direct_path_survives_coordinator_going_away() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();
        let (a_plan, b_plan) =
            drive_simulated(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord).expect("punch");

        // The coordinator is gone. A direct, punched path lives entirely in the
        // two NAT filter states — nothing here consults the coordinator.
        drop(coord);
        assert!(a_nat.allow_inbound(a_plan.local_mapped, b_plan.local_mapped));
        assert!(b_nat.allow_inbound(b_plan.local_mapped, a_plan.local_mapped));
    }
}
