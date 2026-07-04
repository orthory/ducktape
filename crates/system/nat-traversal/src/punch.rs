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

    // 3. Simultaneous open: each side sends a Punch toward the other's
    //    reflexive, opening its own NAT's filter toward that address.
    let _ = a_nat.send(internal(&a_key), a_peer);
    let _ = b_nat.send(internal(&b_key), b_peer);

    // 4. Verify bidirectional reachability.
    if !a_nat.allow_inbound(a_mapped, b_mapped) || !b_nat.allow_inbound(b_mapped, a_mapped) {
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
