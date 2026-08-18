//! the mesh transport's ADDRESS BOOK: `ed25519 key → commonware Address`,
//! with tiered precedence — the application-side replacement for
//! discovery's address gossip now that the mesh rides `authenticated::lookup`
//! (addresses are supplied by the application, never learned on the wire).
//!
//! ## tiers, strongest first
//!
//! 1. **live advert** — a signed reachability record's control endpoint,
//!    observed by the running plane ([`MeshAddressBook::observe_advert`]).
//! 2. **operator hint** — a resolved config dial hint (descriptor
//!    `reach`/`bootstrap` routes, invite-injected ULA routes, dev-shape
//!    `peer_addrs`). EXCEPTION, the named predicate `dns_hint_pins_address`:
//!    a DNS hint outranks live adverts — per-dial re-resolution is
//!    deliberate operator config (the sentry doctrine), and an advert would
//!    freeze it to one stale resolution.
//! 3. **persisted advert** — a mesh-state.json control endpoint from a
//!    previous run; never displaces an operator hint (a possibly-dead
//!    persisted address must not shadow a live operator-provided one).
//! 4. **derived fallthrough** — no entry at all: the peer's overlay ULA at
//!    the default mesh port, a pure function of `(namespace, key)`. this
//!    makes [`MeshAddressBook::addressed`] TOTAL: lookup tracks no peer
//!    without an address, and an undialable derived ULA still keeps the KEY
//!    in the tracked set, so the peer's own inbound dial is accepted
//!    (`bypass_ip_check` + key-in-tracked-set) — the dev-shape joiner, the
//!    endpoint-less coordinated member, and the pre-tunnel overlay member
//!    all connect through their own dials.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::sync::RwLock;

use commonware_cryptography::ed25519;
use commonware_p2p::{Address, Ingress};
use commonware_utils::ordered::{Map, Set};

use crate::overlay_book::ula_of;

/// where an entry's address came from — the precedence discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tier {
    LiveAdvert,
    OperatorHint,
    PersistedAdvert,
}

#[derive(Clone, Debug)]
struct Sourced {
    addr: Address,
    tier: Tier,
}

pub(crate) struct MeshAddressBook {
    /// the genesis namespace string — the ULA derivation input.
    namespace: String,
    /// the port the derived-ULA fallthrough dials (the shipped
    /// `[::]:8846` default; a peer on a custom port is reached through a
    /// hint or an advert instead).
    default_mesh_port: u16,
    entries: RwLock<BTreeMap<ed25519::PublicKey, Sourced>>,
}

impl MeshAddressBook {
    pub(crate) fn new(namespace: impl Into<String>, default_mesh_port: u16) -> Self {
        Self {
            namespace: namespace.into(),
            default_mesh_port,
            entries: RwLock::new(BTreeMap::new()),
        }
    }

    /// seed one operator-config dial hint. replaces persisted entries and
    /// absent ones; a live advert already in place keeps winning UNLESS the
    /// hint is DNS (`dns_hint_pins_address` — the pin outranks adverts in
    /// both directions).
    pub(crate) fn seed_hint(&self, peer: ed25519::PublicKey, ingress: Ingress) {
        let addr = address_of(ingress);
        let mut entries = self.entries.write().expect("mesh book lock");
        let displaces_existing = match entries.get(&peer) {
            None => true,
            Some(existing) => match existing.tier {
                Tier::OperatorHint | Tier::PersistedAdvert => true,
                Tier::LiveAdvert => is_dns(&addr),
            },
        };
        if !displaces_existing {
            return;
        }
        entries.insert(
            peer,
            Sourced {
                addr,
                tier: Tier::OperatorHint,
            },
        );
    }

    /// seed one persisted control endpoint (mesh-state.json). the weakest
    /// written tier: fills only where nothing else is known.
    pub(crate) fn seed_persisted(&self, peer: ed25519::PublicKey, socket: SocketAddr) {
        let mut entries = self.entries.write().expect("mesh book lock");
        if entries.contains_key(&peer) {
            return;
        }
        entries.insert(
            peer,
            Sourced {
                addr: Address::Symmetric(socket),
                tier: Tier::PersistedAdvert,
            },
        );
    }

    /// a live signed advert's control endpoint. returns `Some(addr)` ONLY
    /// when the peer's EFFECTIVE address changed — the caller feeds that to
    /// `AddressableManager::overwrite`, and lookup severs the live
    /// connection on a changed address, so an unchanged re-gossip must stay
    /// silent (this includes an advert that merely confirms the derived
    /// fallthrough).
    pub(crate) fn observe_advert(
        &self,
        peer: &ed25519::PublicKey,
        socket: SocketAddr,
    ) -> Option<Address> {
        let mut entries = self.entries.write().expect("mesh book lock");
        let dns_hint_pins_address = matches!(
            entries.get(peer),
            Some(Sourced { tier: Tier::OperatorHint, addr }) if is_dns(addr)
        );
        if dns_hint_pins_address {
            return None;
        }
        let previous_effective = self.effective_locked(&entries, peer);
        let next = Address::Symmetric(socket);
        let unchanged = previous_effective == next;
        entries.insert(
            peer.clone(),
            Sourced {
                addr: next.clone(),
                tier: Tier::LiveAdvert,
            },
        );
        if unchanged { None } else { Some(next) }
    }

    /// the TOTAL address map for one tracked set, sorted by key (`Set`
    /// iterates ascending, so dedup construction cannot fail).
    pub(crate) fn addressed(&self, peers: &Set<ed25519::PublicKey>) -> Map<ed25519::PublicKey, Address> {
        let entries = self.entries.read().expect("mesh book lock");
        Map::from_iter_dedup(
            peers
                .iter()
                .map(|peer| (peer.clone(), self.effective_locked(&entries, peer))),
        )
    }

    /// one peer's effective address under the lock: its entry, or the
    /// derived-ULA fallthrough.
    fn effective_locked(
        &self,
        entries: &BTreeMap<ed25519::PublicKey, Sourced>,
        peer: &ed25519::PublicKey,
    ) -> Address {
        match entries.get(peer) {
            Some(sourced) => sourced.addr.clone(),
            None => Address::Symmetric(SocketAddr::new(
                IpAddr::V6(ula_of(
                    &self.namespace,
                    peer.as_ref().try_into().expect("ed25519 keys are 32 bytes"),
                )),
                self.default_mesh_port,
            )),
        }
    }
}

/// one resolved config ingress as a lookup `Address`. a DNS ingress keeps
/// per-dial re-resolution via `Address::Asymmetric`; its egress half is a
/// deliberately STABLE unspecified socket — under `bypass_ip_check` the
/// egress is consulted by nothing, and stability keeps repeated seeding from
/// ever reading as an address change.
fn address_of(ingress: Ingress) -> Address {
    match ingress {
        Ingress::Socket(socket) => Address::Symmetric(socket),
        Ingress::Dns { host, port } => Address::Asymmetric {
            ingress: Ingress::Dns { host, port },
            egress: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port),
        },
    }
}

fn is_dns(addr: &Address) -> bool {
    matches!(
        addr,
        Address::Asymmetric {
            ingress: Ingress::Dns { .. },
            ..
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_codec::DecodeExt as _;
    use commonware_cryptography::Signer as _;
    use commonware_cryptography::ed25519::PrivateKey;

    fn key(seed: u8) -> ed25519::PublicKey {
        PrivateKey::decode(&[seed; 32][..])
            .expect("any 32 bytes is a valid seed")
            .public_key()
    }
    fn sock(last: u8, port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(std::net::Ipv4Addr::new(192, 0, 2, last)), port)
    }
    fn book() -> MeshAddressBook {
        MeshAddressBook::new("test#chain", 8846)
    }
    fn set(keys: &[ed25519::PublicKey]) -> Set<ed25519::PublicKey> {
        Set::from_iter_dedup(keys.iter().cloned())
    }

    #[test]
    fn addressed_is_total_via_the_derived_ula_fallthrough() {
        let b = book();
        let peer = key(1);
        let map = b.addressed(&set(std::slice::from_ref(&peer)));
        let Address::Symmetric(addr) = map.get_value(&peer).expect("total").clone() else {
            panic!("fallthrough is a symmetric socket");
        };
        assert_eq!(addr.port(), 8846, "default mesh port");
        assert_eq!(
            addr.ip(),
            IpAddr::V6(ula_of("test#chain", peer.as_ref().try_into().unwrap())),
            "the deterministic ULA"
        );
    }

    #[test]
    fn tier_precedence_advert_over_hint_over_persisted() {
        let b = book();
        let peer = key(1);
        b.seed_persisted(peer.clone(), sock(3, 1000));
        // a hint displaces the persisted entry…
        b.seed_hint(peer.clone(), Ingress::Socket(sock(1, 2000)));
        assert_eq!(
            b.addressed(&set(std::slice::from_ref(&peer))).get_value(&peer).cloned(),
            Some(Address::Symmetric(sock(1, 2000)))
        );
        // …a persisted entry never displaces the hint…
        b.seed_persisted(peer.clone(), sock(4, 3000));
        assert_eq!(
            b.addressed(&set(std::slice::from_ref(&peer))).get_value(&peer).cloned(),
            Some(Address::Symmetric(sock(1, 2000)))
        );
        // …and a live advert displaces the hint.
        let changed = b.observe_advert(&peer, sock(2, 4000));
        assert_eq!(changed, Some(Address::Symmetric(sock(2, 4000))));
        assert_eq!(
            b.addressed(&set(std::slice::from_ref(&peer))).get_value(&peer).cloned(),
            Some(Address::Symmetric(sock(2, 4000)))
        );
    }

    #[test]
    fn observe_advert_is_silent_when_the_effective_address_is_unchanged() {
        let b = book();
        let peer = key(1);
        assert!(b.observe_advert(&peer, sock(1, 9000)).is_some(), "first move fires");
        assert!(
            b.observe_advert(&peer, sock(1, 9000)).is_none(),
            "re-gossip of the same endpoint is silent"
        );
        assert!(b.observe_advert(&peer, sock(2, 9000)).is_some(), "a move fires");
    }

    #[test]
    fn advert_confirming_the_derived_fallthrough_is_silent() {
        let b = book();
        let peer = key(1);
        let derived = SocketAddr::new(
            IpAddr::V6(ula_of("test#chain", peer.as_ref().try_into().unwrap())),
            8846,
        );
        assert!(
            b.observe_advert(&peer, derived).is_none(),
            "the advert only confirms what the book already answers — no overwrite churn"
        );
    }

    #[test]
    fn dns_hint_pins_the_address_against_adverts() {
        let b = book();
        let peer = key(1);
        let dns = Ingress::Dns {
            host: "sentry.example.com".try_into().expect("hostname"),
            port: 443,
        };
        b.seed_hint(peer.clone(), dns.clone());
        assert!(
            b.observe_advert(&peer, sock(1, 9000)).is_none(),
            "a live advert never displaces a DNS hint (per-dial re-resolution is deliberate)"
        );
        let expected = address_of(dns);
        assert_eq!(b.addressed(&set(std::slice::from_ref(&peer))).get_value(&peer).cloned(), Some(expected));
    }

    #[test]
    fn dns_hint_egress_is_stable_across_reseeding() {
        let dns = || Ingress::Dns {
            host: "edge.example.com".try_into().expect("hostname"),
            port: 443,
        };
        assert_eq!(
            address_of(dns()),
            address_of(dns()),
            "repeated seeding must never read as an address change"
        );
    }
}
