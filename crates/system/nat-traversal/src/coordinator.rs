use std::collections::HashMap;
use std::net::SocketAddr;

use crate::{Msg, NodeKey};

/// The untrusted entry helper. Maps a node key to the reflexive address the
/// coordinator observed for it, and brokers a simultaneous-open. Holds no key
/// material, no plaintext, no mesh authority.
#[derive(Default)]
pub struct Coordinator {
    reflexive: HashMap<NodeKey, SocketAddr>,
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
                self.reflexive.insert(key, from);
                Vec::new()
            }
            Msg::Lookup { key } => {
                let target = self.reflexive.get(&key).copied();
                let mut out = vec![(from, Msg::LookupResponse { key, reflexive: target })];
                if let Some(peer_addr) = target {
                    // Find the caller's own key by reverse-mapping its source;
                    // fall back to a zero key if it never registered (still lets
                    // the target learn the caller's reflexive to punch back).
                    let caller_key = self
                        .reflexive
                        .iter()
                        .find(|&(_, &v)| v == from)
                        .map(|(k, _)| *k)
                        .unwrap_or(NodeKey([0u8; 32]));
                    out.push((from, Msg::PunchSync { peer: key, peer_reflexive: peer_addr }));
                    out.push((peer_addr, Msg::PunchSync { peer: caller_key, peer_reflexive: from }));
                }
                out
            }
            // The coordinator never routes these through `handle`:
            // BindResponse/LookupResponse/PunchSync/Punch are node-directed;
            // RelayRequest is intercepted by the async loop (it must bind
            // sockets); RelayGrant is node-directed. Ignore defensively.
            Msg::BindResponse { .. }
            | Msg::LookupResponse { .. }
            | Msg::PunchSync { .. }
            | Msg::Punch { .. }
            | Msg::RelayRequest { .. }
            | Msg::RelayGrant { .. } => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Msg, NodeKey};
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(203, 0, 113, o)), p)
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
