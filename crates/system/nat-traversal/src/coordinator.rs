use std::net::SocketAddr;

use crate::advert::{AdvertBook, AdvertOutcome};
use crate::{Msg, NodeKey};

/// The untrusted entry helper. Maps a node key to the reflexive address the
/// coordinator observed for it, and brokers a simultaneous-open. Holds no key
/// material, no plaintext, no mesh authority — and never carries peer traffic:
/// rendezvous only, no relay.
#[derive(Default)]
pub struct Coordinator {
    adverts: AdvertBook,
}

impl Coordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Handle one datagram observed from `from`; return datagrams to send.
    pub fn handle(&mut self, from: SocketAddr, msg: Msg) -> Vec<(SocketAddr, Msg)> {
        match msg {
            Msg::BindRequest { .. } => {
                vec![(from, Msg::BindResponse { reflexive: from })]
            }
            Msg::Register { key } => {
                // The registered reflexive address IS the observed source: the
                // coordinator never trusts a self-reported address.
                self.adverts.observe(key, from);
                Vec::new()
            }
            Msg::Readvertise { key, nonce } => {
                // The wire-level rebind path: a NAT-rebound node re-runs STUN and
                // republishes its NEW reflexive (the observed `from`, never a
                // self-reported address) under a strictly-higher `nonce`. The
                // `AdvertBook` staleness guard rejects an equal-or-lower nonce, so
                // a replayed/reordered datagram cannot supersede a fresh mapping.
                self.adverts.readvertise(key, from, nonce);
                Vec::new()
            }
            Msg::Lookup { key } => {
                let target = self.adverts.current(key);
                let mut out = vec![(from, Msg::LookupResponse { key, reflexive: target })];
                if let Some(peer_addr) = target {
                    // Find the caller's own key by reverse-mapping its source;
                    // fall back to a zero key if it never registered (still lets
                    // the target learn the caller's reflexive to punch back).
                    let caller_key = self.adverts.key_for_src(from).unwrap_or(NodeKey([0u8; 32]));
                    out.push((from, Msg::PunchSync { peer: key, peer_reflexive: peer_addr }));
                    out.push((peer_addr, Msg::PunchSync { peer: caller_key, peer_reflexive: from }));
                }
                out
            }
            // The coordinator never routes these through `handle`:
            // BindResponse/LookupResponse/PunchSync/Punch are node-directed.
            // Ignore defensively.
            Msg::BindResponse { .. }
            | Msg::LookupResponse { .. }
            | Msg::PunchSync { .. }
            | Msg::Punch { .. } => Vec::new(),
        }
    }

    /// Reachability-plane rebind re-advertisement. A node whose NAT rebound
    /// re-runs STUN (its datagram is observed from a NEW source) and calls this
    /// under a strictly-higher `nonce` to supersede its stale reflexive; an
    /// equal-or-lower nonce is rejected as stale (a replay cannot clobber the
    /// fresh mapping). After a `Superseded`, a peer's `Lookup` resolves the new
    /// reflexive.
    pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome {
        self.adverts.readvertise(key, src, nonce)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AdvertOutcome, Msg, NodeKey};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, o)), p)
    }

    #[test]
    fn readvertise_supersedes_stale_mapping_and_lookup_reflects_it() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);
        let a = NodeKey([0xaa; 32]);
        let b = NodeKey([0xbb; 32]);
        c.handle(a_src, Msg::Register { key: a });
        c.handle(b_src, Msg::Register { key: b });

        // A rebinds to a new reflexive and re-advertises under a higher nonce.
        let a_new = addr(1, 9999);
        assert_eq!(c.readvertise(a, a_new, 1), AdvertOutcome::Superseded);

        // B's lookup now resolves A's NEW reflexive, and the fan-out PunchSync to
        // A targets the new mapping.
        let out = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(a_new) })));
        assert!(out.contains(&(a_new, Msg::PunchSync { peer: b, peer_reflexive: b_src })));

        // A replayed/equal-nonce re-advert is stale and does not move the mapping.
        assert_eq!(c.readvertise(a, addr(1, 7777), 1), AdvertOutcome::Stale);
        let out2 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out2.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(a_new) })));
    }

    #[test]
    fn wire_readvertise_supersedes_and_replayed_register_cannot_roll_it_back() {
        // Everything here goes through `handle` — the SAME dispatch the real UDP
        // loop uses — so this proves the nonce-gated rebind is reachable over the
        // wire protocol, not only via the in-process `readvertise` API.
        let mut c = Coordinator::new();
        let a = NodeKey([0xaa; 32]);
        let b_src = addr(2, 2222);
        let old = addr(1, 1111);
        let new = addr(1, 9999);

        // Boot: A registers from its old mapping (implicit nonce 0).
        assert!(c.handle(old, Msg::Register { key: a }).is_empty());

        // A rebinds and re-advertises the NEW mapping over the wire under nonce 1.
        assert!(c.handle(new, Msg::Readvertise { key: a, nonce: 1 }).is_empty());
        let out = c.handle(b_src, Msg::Lookup { key: a });
        assert!(
            out.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(new) })),
            "a wire Readvertise supersedes the stale mapping"
        );

        // A duplicated/reordered/replayed Register from the OLD mapping arrives
        // late. It must NOT roll the fresh {new, nonce=1} mapping back to old.
        assert!(c.handle(old, Msg::Register { key: a }).is_empty());
        let out2 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(
            out2.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(new) })),
            "a replayed nonce-0 Register must not clobber a higher-nonce readvertised mapping"
        );

        // A wire Readvertise at an equal-or-lower nonce is likewise stale.
        assert!(c.handle(old, Msg::Readvertise { key: a, nonce: 1 }).is_empty());
        let out3 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out3.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(new) })));
    }

    #[test]
    fn bind_request_echoes_observed_source() {
        let mut c = Coordinator::new();
        let src = addr(7, 40000);
        let out = c.handle(src, Msg::BindRequest { from: NodeKey([1u8; 32]) });
        assert_eq!(out, vec![(src, Msg::BindResponse { reflexive: src })]);
    }

    #[test]
    fn register_then_lookup_returns_reflexive() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);
        let a = NodeKey([0xaa; 32]);
        let b = NodeKey([0xbb; 32]);
        assert!(c.handle(a_src, Msg::Register { key: a }).is_empty());
        assert!(c.handle(b_src, Msg::Register { key: b }).is_empty());

        // A looks up B: coordinator replies to A with B's reflexive AND tells
        // both sides to punch simultaneously.
        let out = c.handle(a_src, Msg::Lookup { key: b });
        assert!(out.contains(&(a_src, Msg::LookupResponse { key: b, reflexive: Some(b_src) })));
        assert!(out.contains(&(a_src, Msg::PunchSync { peer: b, peer_reflexive: b_src })));
        assert!(out.contains(&(b_src, Msg::PunchSync { peer: a, peer_reflexive: a_src })));
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let missing = NodeKey([0xcc; 32]);
        let out = c.handle(a_src, Msg::Lookup { key: missing });
        assert_eq!(out, vec![(a_src, Msg::LookupResponse { key: missing, reflexive: None })]);
    }
}
