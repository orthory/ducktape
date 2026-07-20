use std::collections::{HashMap, HashSet};
use std::net::{IpAddr, SocketAddr};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mapping {
    /// Endpoint-independent: one stable public port per internal socket.
    Cone,
    /// Address-dependent: a fresh public port per (internal, destination).
    Symmetric,
}

/// A NAT model. `new` is restricted-cone (endpoint-independent mapping,
/// address-dependent filtering) — the case simultaneous-open hole-punch
/// targets. `symmetric` is the case hole-punch cannot beat, forcing relay
/// fallback: a different public port per destination, so the coordinator-
/// observed reflexive port never admits a peer's punch.
pub struct SimNat {
    public_ip: IpAddr,
    next_port: u16,
    mode: Mapping,
    cone: HashMap<SocketAddr, SocketAddr>, // internal -> public (endpoint-independent)
    sym: HashMap<(SocketAddr, SocketAddr), SocketAddr>, // (internal, dst) -> public
    holes: HashSet<(SocketAddr, SocketAddr)>, // (public mapped, remote) opened
}

impl SimNat {
    pub fn new(public_ip: IpAddr) -> Self {
        Self {
            public_ip,
            next_port: 1024,
            mode: Mapping::Cone,
            cone: HashMap::new(),
            sym: HashMap::new(),
            holes: HashSet::new(),
        }
    }

    pub fn symmetric(public_ip: IpAddr) -> Self {
        Self {
            mode: Mapping::Symmetric,
            ..Self::new(public_ip)
        }
    }

    /// Model a NAT rebinding: the device drops its current mappings and holes
    /// (lease expiry, reboot, or mapping timeout). The next outbound datagram
    /// from an internal socket allocates a FRESH public port — `next_port` never
    /// rewinds, so the new reflexive is guaranteed distinct — and the old
    /// mapping admits nobody, so a peer still aimed at the stale reflexive fails.
    /// This is the trigger for STUN re-run + higher-nonce re-advertisement.
    pub fn rebind(&mut self) {
        self.cone.clear();
        self.sym.clear();
        self.holes.clear();
    }

    fn alloc_port(&mut self) -> u16 {
        let port = self.next_port;
        self.next_port = self.next_port.wrapping_add(1).max(1024);
        port
    }

    /// Record an outbound datagram from `internal_src` toward `dst`; return the
    /// public source address peers will observe.
    pub fn send(&mut self, internal_src: SocketAddr, dst: SocketAddr) -> SocketAddr {
        let mapped = match self.mode {
            Mapping::Cone => {
                if let Some(&m) = self.cone.get(&internal_src) {
                    m
                } else {
                    let m = SocketAddr::new(self.public_ip, self.alloc_port());
                    self.cone.insert(internal_src, m);
                    m
                }
            }
            Mapping::Symmetric => {
                if let Some(&m) = self.sym.get(&(internal_src, dst)) {
                    m
                } else {
                    let m = SocketAddr::new(self.public_ip, self.alloc_port());
                    self.sym.insert((internal_src, dst), m);
                    m
                }
            }
        };
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
        assert!(
            !nat.allow_inbound(mapped, peer),
            "unsolicited inbound is dropped"
        );
        let _ = nat.send(internal, peer); // now punch toward peer
        assert!(nat.allow_inbound(mapped, peer), "hole toward peer now open");
    }

    #[test]
    fn rebind_moves_the_reflexive_and_invalidates_the_old_mapping() {
        let mut nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let peer = a([198, 51, 100, 2], 51820);

        let old = nat.send(internal, a([192, 0, 2, 1], 3478)); // reflexive toward coordinator
        let _ = nat.send(internal, peer); // punch a hole toward the peer
        assert!(
            nat.allow_inbound(old, peer),
            "hole toward peer is open pre-rebind"
        );

        // The NAT rebinds: mapping + holes are dropped.
        nat.rebind();

        // The next STUN send yields a DIFFERENT reflexive (the stale mapping is
        // superseded), and the old mapping admits nobody anymore.
        let new = nat.send(internal, a([192, 0, 2, 1], 3478));
        assert_ne!(old, new, "rebind must move the reflexive to a fresh port");
        assert!(
            !nat.allow_inbound(old, peer),
            "the old mapping no longer admits the peer"
        );
    }

    #[test]
    fn rebind_moves_the_reflexive_for_symmetric_too() {
        let mut nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let coord = a([192, 0, 2, 1], 3478);
        let old = nat.send(internal, coord);
        nat.rebind();
        let new = nat.send(internal, coord);
        assert_ne!(
            old, new,
            "symmetric rebind also moves the coordinator-facing mapping"
        );
    }

    #[test]
    fn symmetric_allocates_a_fresh_port_per_destination() {
        let mut nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let to_coord = nat.send(internal, a([192, 0, 2, 1], 3478));
        let to_peer = nat.send(internal, a([198, 51, 100, 2], 6000));
        assert_ne!(
            to_coord, to_peer,
            "symmetric NAT maps a different public port per destination"
        );
        // The port the coordinator observed does NOT admit the peer: the peer
        // would punch `to_coord`, but only `to_peer` opened a hole toward it.
        assert!(!nat.allow_inbound(to_coord, a([198, 51, 100, 2], 6000)));
    }
}
