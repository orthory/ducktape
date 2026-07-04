use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};

/// Endpoint-independent mapping, address-and-port-dependent filtering NAT
/// (restricted-cone). One public IP; each internal socket gets a stable public
/// port; inbound is allowed only from destinations this internal socket has
/// already sent to. This is the case simultaneous-open hole-punch targets.
pub struct SimNat {
    public_ip: IpAddr,
    next_port: u16,
    mapping: HashMap<SocketAddr, SocketAddr>, // internal -> public
    holes: HashSet<(SocketAddr, SocketAddr)>, // (public mapped, remote) opened
}

impl SimNat {
    pub fn new(public_ip: IpAddr) -> Self {
        Self {
            public_ip,
            next_port: 1024,
            mapping: HashMap::new(),
            holes: HashSet::new(),
        }
    }

    /// Record an outbound datagram from `internal_src` toward `dst`; return the
    /// public source address peers will observe.
    pub fn send(&mut self, internal_src: SocketAddr, dst: SocketAddr) -> SocketAddr {
        let public_ip = self.public_ip;
        let next = &mut self.next_port;
        let mapped = *self.mapping.entry(internal_src).or_insert_with(|| {
            let port = *next;
            *next = next.wrapping_add(1).max(1024);
            SocketAddr::new(public_ip, port)
        });
        self.holes.insert((mapped, dst));
        mapped
    }

    /// May an inbound datagram from `from` reach the internal socket behind
    /// `mapped`? Only if a prior outbound opened a hole toward `from`.
    pub fn allow_inbound(&self, mapped: SocketAddr, from: SocketAddr) -> bool {
        self.holes.contains(&(mapped, from))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn a(ip: [u8; 4], p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::from(ip)), p)
    }

    #[test]
    fn mapping_is_endpoint_independent_and_stable() {
        let mut nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let m1 = nat.send(internal, a([203, 0, 113, 9], 40000));
        let m2 = nat.send(internal, a([203, 0, 113, 10], 50000));
        assert_eq!(m1, m2, "same internal socket -> same public mapping");
        assert_eq!(m1.ip(), IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
    }

    #[test]
    fn inbound_filtered_until_outbound_opens_hole() {
        let mut nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let peer = a([198, 51, 100, 2], 51820);
        let mapped = nat.send(internal, a([203, 0, 113, 9], 40000)); // hole to coordinator only
        assert!(!nat.allow_inbound(mapped, peer), "unsolicited inbound is dropped");
        let _ = nat.send(internal, peer); // now punch toward peer
        assert!(nat.allow_inbound(mapped, peer), "hole toward peer now open");
    }
}
