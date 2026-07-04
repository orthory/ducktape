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
    //    actually delivers it.
    let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
    let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };
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

    // 4. Verify bidirectional reachability was actually observed, not just
    //    assumed from final filter state.
    if !a_delivered || !b_delivered {
        return Err(PunchError::NotReachable);
    }

    Ok((
        PunchPlan { local_mapped: a_mapped, peer_reflexive: a_peer },
        PunchPlan { local_mapped: b_mapped, peer_reflexive: b_peer },
    ))
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
}
