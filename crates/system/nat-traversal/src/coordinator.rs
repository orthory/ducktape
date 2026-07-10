use std::net::SocketAddr;
use std::num::NonZeroUsize;

use arrayvec::ArrayVec;
use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use lru::LruCache;

use crate::AuthRequest;
use crate::advert::{AdvertBook, AdvertOutcome};
use crate::auth::{AuthPolicy, DEFAULT_FRESHNESS_WINDOW_SECS, verify_request_using};
use crate::{Msg, NodeKey};

const AUTH_KEY_CACHE_SIZE: NonZeroUsize = NonZeroUsize::new(64).unwrap();
type AuthKeyCache = Option<LruCache<NodeKey, ed25519::PublicKey>>;

/// Small LRU for parsed ed25519 caller keys. Parsing a valid key is roughly a
/// tenth of request verification cost. It allocates lazily, so the fully-open
/// legacy coordinator pays nothing, and the library-enforced capacity keeps
/// attacker-selected callers from growing memory without bound.
fn resolve_auth_key(cache: &mut AuthKeyCache, key: NodeKey) -> Option<ed25519::PublicKey> {
    if let Some(cached) = cache.as_mut().and_then(|entries| entries.get(&key)) {
        return Some(cached.clone());
    }

    let parsed = ed25519::PublicKey::decode(key.0.as_slice()).ok()?;
    cache
        .get_or_insert_with(|| LruCache::new(AUTH_KEY_CACHE_SIZE))
        .put(key, parsed.clone());
    Some(parsed)
}

pub type CoordinatorReply = (SocketAddr, Msg);

/// Allocation-free output from the coordinator's bounded request handler.
/// One request can produce at most three datagrams: the lookup response and a
/// `PunchSync` for each peer. The compatibility handlers still return `Vec`s;
/// the live UDP loop consumes this fixed-capacity form directly.
pub type CoordinatorReplies = ArrayVec<CoordinatorReply, 3>;

/// The untrusted entry helper. Maps a node key to the reflexive address the
/// coordinator observed for it, and brokers a simultaneous-open. Holds no key
/// material, no plaintext, no mesh authority — and never carries peer traffic:
/// rendezvous only, no relay.
pub struct Coordinator {
    adverts: AdvertBook,
    policy: AuthPolicy,
    auth_keys: AuthKeyCache,
    window: u64,
    rejects: u64,
}

impl Default for Coordinator {
    fn default() -> Self {
        Self {
            adverts: AdvertBook::default(),
            policy: AuthPolicy::default(), // fully-open
            auth_keys: None,
            window: DEFAULT_FRESHNESS_WINDOW_SECS,
            rejects: 0,
        }
    }
}

impl Coordinator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct with an explicit authorization policy.
    pub fn with_policy(policy: AuthPolicy) -> Self {
        Self {
            policy,
            ..Self::default()
        }
    }

    /// Construct with an explicit policy AND registration TTL (seconds). The
    /// default is [`crate::advert::REGISTRATION_TTL_SECS`]; tests and
    /// short-lived rigs shrink it.
    pub fn with_policy_and_ttl(policy: AuthPolicy, ttl_secs: u64) -> Self {
        Self {
            adverts: AdvertBook::with_ttl(ttl_secs),
            ..Self::with_policy(policy)
        }
    }

    /// Count of requests dropped by the auth gate (observability).
    pub fn rejects(&self) -> u64 {
        self.rejects
    }

    /// Authenticate then handle one authenticated request. `now` is wall-clock
    /// seconds. A failed authenticator produces NO reply and bumps the counter.
    pub fn handle_auth(
        &mut self,
        from: SocketAddr,
        req: AuthRequest,
        now: u64,
    ) -> Vec<(SocketAddr, Msg)> {
        self.handle_auth_replies(from, req, now)
            .into_iter()
            .collect()
    }

    /// Allocation-free authenticated handler used by the live UDP loop.
    pub fn handle_auth_replies(
        &mut self,
        from: SocketAddr,
        req: AuthRequest,
        now: u64,
    ) -> CoordinatorReplies {
        // Only request shapes are wrappable; a non-request inner is malformed.
        let Some(inner_subject) = req.inner.subject_key() else {
            self.rejects += 1;
            return CoordinatorReplies::new();
        };
        // Anti-poisoning: for the SELF-ops (BindRequest/Register/Readvertise) the
        // inner key IS the acting node, so it must equal the authenticated
        // caller — otherwise an admitted member could register or re-advertise
        // ANOTHER node's key. A `Lookup` has no such constraint: a member looks
        // up a DIFFERENT peer, so its inner key is expected to differ.
        let is_self_op = !matches!(req.inner, Msg::Lookup { .. });
        if is_self_op && inner_subject != req.caller {
            self.rejects += 1;
            return CoordinatorReplies::new();
        }
        let inner_bytes = req.inner.encode_inline();
        // Authenticate the CALLER (the PoP-signing identity), not the inner key.
        let policy = &self.policy;
        let auth_keys = &mut self.auth_keys;
        match verify_request_using(
            policy,
            now,
            self.window,
            req.caller,
            &inner_bytes,
            &req.auth,
            |caller| resolve_auth_key(auth_keys, caller),
        ) {
            Ok(()) => self.handle_with_caller_replies(from, req.inner, Some(req.caller), now),
            Err(_) => {
                self.rejects += 1;
                CoordinatorReplies::new()
            }
        }
    }

    /// Handle a legacy (unauthenticated) request at wall-clock `now` (seconds).
    /// Accepted ONLY under the fully-open policy; any auth-requiring policy
    /// drops it.
    pub fn handle_legacy(
        &mut self,
        from: SocketAddr,
        msg: Msg,
        now: u64,
    ) -> Vec<(SocketAddr, Msg)> {
        self.handle_legacy_replies(from, msg, now)
            .into_iter()
            .collect()
    }

    /// Allocation-free legacy handler used by the live UDP loop.
    pub fn handle_legacy_replies(
        &mut self,
        from: SocketAddr,
        msg: Msg,
        now: u64,
    ) -> CoordinatorReplies {
        if matches!(self.policy, AuthPolicy::Open { require_pop: false }) {
            self.handle_with_caller_replies(from, msg, None, now)
        } else {
            self.rejects += 1;
            CoordinatorReplies::new()
        }
    }

    /// Handle one datagram observed from `from` at wall-clock `now` (seconds);
    /// return datagrams to send. The time-threading is what lets registrations
    /// EXPIRE: a mapping older than the TTL resolves to `None` and never
    /// receives a `PunchSync` fan-out (its NAT pinhole is long dead). The
    /// caller identity is unknown on this path, so a `Lookup`'s PunchSync
    /// fan-out reverse-maps the source (through the private
    /// `handle_with_caller_replies` helper).
    pub fn handle_at(&mut self, from: SocketAddr, msg: Msg, now: u64) -> Vec<(SocketAddr, Msg)> {
        self.handle_with_caller_replies(from, msg, None, now)
            .into_iter()
            .collect()
    }

    /// Time-frozen convenience (`now = 0`) for the deterministic sims/tests,
    /// where no wall time passes between registration and lookup.
    pub fn handle(&mut self, from: SocketAddr, msg: Msg) -> Vec<(SocketAddr, Msg)> {
        self.handle_with_caller_replies(from, msg, None, 0)
            .into_iter()
            .collect()
    }

    /// The pure handler. `caller`, when `Some`, is the AUTHENTICATED requesting
    /// identity — used for a `Lookup`'s peer-directed `PunchSync` so the passive
    /// side learns the caller's real key even before the caller has registered.
    /// When `None` (legacy path) the caller key is reverse-mapped from the
    /// datagram source, falling back to a zero key.
    fn handle_with_caller_replies(
        &mut self,
        from: SocketAddr,
        msg: Msg,
        caller: Option<NodeKey>,
        now: u64,
    ) -> CoordinatorReplies {
        match msg {
            Msg::BindRequest { .. } => {
                CoordinatorReplies::from_iter([(from, Msg::BindResponse { reflexive: from })])
            }
            Msg::Register { key } => {
                // The registered reflexive address IS the observed source: the
                // coordinator never trusts a self-reported address.
                self.adverts.observe(key, from, now);
                CoordinatorReplies::new()
            }
            Msg::Readvertise { key, nonce } => {
                // The wire-level rebind path AND the keepalive: a node re-runs
                // STUN and republishes its reflexive (the observed `from`, never
                // a self-reported address) under a strictly-higher `nonce`. The
                // `AdvertBook` staleness guard rejects an equal-or-lower nonce, so
                // a replayed/reordered datagram cannot supersede a fresh mapping
                // — nor extend its life.
                self.adverts.readvertise(key, from, nonce, now);
                CoordinatorReplies::new()
            }
            Msg::Lookup { key } => {
                let target = self.adverts.current(key, now);
                let response = (
                    from,
                    Msg::LookupResponse {
                        key,
                        reflexive: target,
                    },
                );
                if let Some(peer_addr) = target {
                    // The caller's key for the peer-directed PunchSync: the
                    // AUTHENTICATED caller when we have one, else reverse-map the
                    // datagram source (legacy path), falling back to a zero key
                    // if it never registered (the target still learns the
                    // caller's reflexive to punch back).
                    let caller_key = caller
                        .or_else(|| self.adverts.key_for_src(from, now))
                        .unwrap_or(NodeKey([0u8; 32]));
                    CoordinatorReplies::from([
                        response,
                        (
                            from,
                            Msg::PunchSync {
                                peer: key,
                                peer_reflexive: peer_addr,
                            },
                        ),
                        (
                            peer_addr,
                            Msg::PunchSync {
                                peer: caller_key,
                                peer_reflexive: from,
                            },
                        ),
                    ])
                } else {
                    CoordinatorReplies::from_iter([response])
                }
            }
            // The coordinator never routes these through `handle`:
            // BindResponse/LookupResponse/PunchSync/Punch are node-directed.
            // Ignore defensively.
            Msg::BindResponse { .. }
            | Msg::LookupResponse { .. }
            | Msg::PunchSync { .. }
            | Msg::Punch { .. } => CoordinatorReplies::new(),
        }
    }

    /// Reachability-plane rebind re-advertisement. A node whose NAT rebound
    /// re-runs STUN (its datagram is observed from a NEW source) and calls this
    /// under a strictly-higher `nonce` to supersede its stale reflexive; an
    /// equal-or-lower nonce is rejected as stale (a replay cannot clobber the
    /// fresh mapping). After a `Superseded`, a peer's `Lookup` resolves the new
    /// reflexive.
    pub fn readvertise(
        &mut self,
        key: NodeKey,
        src: SocketAddr,
        nonce: u64,
        now: u64,
    ) -> AdvertOutcome {
        self.adverts.readvertise(key, src, nonce, now)
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
        assert_eq!(c.readvertise(a, a_new, 1, 0), AdvertOutcome::Superseded);

        // B's lookup now resolves A's NEW reflexive, and the fan-out PunchSync to
        // A targets the new mapping.
        let out = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out.contains(&(
            b_src,
            Msg::LookupResponse {
                key: a,
                reflexive: Some(a_new)
            }
        )));
        assert!(out.contains(&(
            a_new,
            Msg::PunchSync {
                peer: b,
                peer_reflexive: b_src
            }
        )));

        // A replayed/equal-nonce re-advert is stale and does not move the mapping.
        assert_eq!(c.readvertise(a, addr(1, 7777), 1, 0), AdvertOutcome::Stale);
        let out2 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out2.contains(&(
            b_src,
            Msg::LookupResponse {
                key: a,
                reflexive: Some(a_new)
            }
        )));
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
        assert!(
            c.handle(new, Msg::Readvertise { key: a, nonce: 1 })
                .is_empty()
        );
        let out = c.handle(b_src, Msg::Lookup { key: a });
        assert!(
            out.contains(&(
                b_src,
                Msg::LookupResponse {
                    key: a,
                    reflexive: Some(new)
                }
            )),
            "a wire Readvertise supersedes the stale mapping"
        );

        // A duplicated/reordered/replayed Register from the OLD mapping arrives
        // late. It must NOT roll the fresh {new, nonce=1} mapping back to old.
        assert!(c.handle(old, Msg::Register { key: a }).is_empty());
        let out2 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(
            out2.contains(&(
                b_src,
                Msg::LookupResponse {
                    key: a,
                    reflexive: Some(new)
                }
            )),
            "a replayed nonce-0 Register must not clobber a higher-nonce readvertised mapping"
        );

        // A wire Readvertise at an equal-or-lower nonce is likewise stale.
        assert!(
            c.handle(old, Msg::Readvertise { key: a, nonce: 1 })
                .is_empty()
        );
        let out3 = c.handle(b_src, Msg::Lookup { key: a });
        assert!(out3.contains(&(
            b_src,
            Msg::LookupResponse {
                key: a,
                reflexive: Some(new)
            }
        )));
    }

    #[test]
    fn bind_request_echoes_observed_source() {
        let mut c = Coordinator::new();
        let src = addr(7, 40000);
        let out = c.handle(
            src,
            Msg::BindRequest {
                from: NodeKey([1u8; 32]),
            },
        );
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
        assert!(out.contains(&(
            a_src,
            Msg::LookupResponse {
                key: b,
                reflexive: Some(b_src)
            }
        )));
        assert!(out.contains(&(
            a_src,
            Msg::PunchSync {
                peer: b,
                peer_reflexive: b_src
            }
        )));
        assert!(out.contains(&(
            b_src,
            Msg::PunchSync {
                peer: a,
                peer_reflexive: a_src
            }
        )));
    }

    #[test]
    fn lookup_unknown_returns_none() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let missing = NodeKey([0xcc; 32]);
        let out = c.handle(a_src, Msg::Lookup { key: missing });
        assert_eq!(
            out,
            vec![(
                a_src,
                Msg::LookupResponse {
                    key: missing,
                    reflexive: None
                }
            )]
        );
    }

    #[test]
    fn private_policy_admits_authorized_register_and_lookup_but_drops_unauthorized() {
        use crate::AuthRequest;
        use crate::auth::{AuthPolicy, mint_coord_cap, now_secs, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let g = ed25519::PrivateKey::from_seed(100);
        let node = ed25519::PrivateKey::from_seed(200);
        let mut nb = [0u8; 32];
        nb.copy_from_slice(node.public_key().as_ref());
        let subject = NodeKey(nb);
        let now = now_secs();

        let mut c = Coordinator::with_policy(AuthPolicy::Private {
            genesis_set: vec![g.public_key()],
        });
        let src = addr(1, 1111);

        // Authorized: joiner with a valid genesis cap registers -> mapping created.
        let reg = Msg::Register { key: subject };
        let cap = mint_coord_cap(&g, subject, now + 3600);
        let auth = sign_authenticator(&node, &reg.encode(), now, Some(cap));
        let out = c.handle_auth(
            src,
            AuthRequest {
                caller: subject,
                inner: reg,
                auth,
            },
            now,
        );
        assert!(out.is_empty());
        // A lookup from the same authorized node resolves it. The caller is the
        // authenticated principal; here it looks up its OWN key.
        let lk = Msg::Lookup { key: subject };
        let lauth = sign_authenticator(
            &node,
            &lk.encode(),
            now,
            Some(mint_coord_cap(&g, subject, now + 3600)),
        );
        let out = c.handle_auth(
            src,
            AuthRequest {
                caller: subject,
                inner: lk,
                auth: lauth,
            },
            now,
        );
        assert!(out.iter().any(|(_, m)| matches!(
            m,
            Msg::LookupResponse {
                reflexive: Some(_),
                ..
            }
        )));

        // Unauthorized: outsider (no cap) -> dropped, no mapping, reject counted.
        let outsider = ed25519::PrivateKey::from_seed(201);
        let mut ob = [0u8; 32];
        ob.copy_from_slice(outsider.public_key().as_ref());
        let osub = NodeKey(ob);
        let before = c.rejects();
        let oreg = Msg::Register { key: osub };
        let oauth = sign_authenticator(&outsider, &oreg.encode(), now, None);
        let out = c.handle_auth(
            addr(2, 2222),
            AuthRequest {
                caller: osub,
                inner: oreg,
                auth: oauth,
            },
            now,
        );
        assert!(out.is_empty());
        assert_eq!(c.rejects(), before + 1);
        // The outsider's key never entered the book: an AUTHORIZED lookup for it
        // resolves to None (the dropped register created no mapping). The
        // outsider here holds a genesis cap, so it authenticates as the caller
        // and looks up its own (unregistered) key.
        let lk = Msg::Lookup { key: osub };
        let lauth = sign_authenticator(
            &outsider,
            &lk.encode(),
            now,
            Some(mint_coord_cap(&g, osub, now + 3600)),
        );
        let out = c.handle_auth(
            src,
            AuthRequest {
                caller: osub,
                inner: lk,
                auth: lauth,
            },
            now,
        );
        assert!(out.iter().any(|(_, m)| matches!(
            m,
            Msg::LookupResponse {
                reflexive: None,
                ..
            }
        )));
    }

    #[test]
    fn parsed_key_cache_is_lazy_and_never_skips_signature_verification() {
        use crate::AuthRequest;
        use crate::auth::{AuthPolicy, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let node = ed25519::PrivateKey::from_seed(250);
        let attacker = ed25519::PrivateKey::from_seed(251);
        let mut key = [0; 32];
        key.copy_from_slice(node.public_key().as_ref());
        let caller = NodeKey(key);
        let source = addr(1, 1111);
        let now = 1_000_000;
        let inner = Msg::BindRequest { from: caller };
        let mut coordinator = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });

        assert!(coordinator.auth_keys.is_none(), "cache starts lazy");
        let good = AuthRequest {
            caller,
            inner: inner.clone(),
            auth: sign_authenticator(&node, &inner.encode(), now, None),
        };
        assert_eq!(coordinator.handle_auth_replies(source, good, now).len(), 1);
        assert!(coordinator.auth_keys.is_some(), "valid key was cached");

        // This hits the cached parsed public key but carries a signature from a
        // different private key. The cache saves only key parsing; it must not
        // cache or bypass the per-request proof-of-possession decision.
        let forged = AuthRequest {
            caller,
            inner: inner.clone(),
            auth: sign_authenticator(&attacker, &inner.encode(), now, None),
        };
        let before = coordinator.rejects();
        assert!(
            coordinator
                .handle_auth_replies(source, forged, now)
                .is_empty()
        );
        assert_eq!(coordinator.rejects(), before + 1);
    }

    #[test]
    fn cross_peer_lookup_authenticates_the_caller_and_fans_out() {
        // The previously-impossible path: caller A (admitted) looks up a
        // DIFFERENT peer B. Authentication is against A's key (the caller), so
        // A's PoP — signed with A's own key — validates, and the coordinator
        // returns B's mapping plus a PunchSync fan-out carrying A's REAL key.
        use crate::AuthRequest;
        use crate::auth::{AuthPolicy, mint_coord_cap, now_secs, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let g = ed25519::PrivateKey::from_seed(300);
        let a = ed25519::PrivateKey::from_seed(301);
        let b = ed25519::PrivateKey::from_seed(302);
        let a_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(a.public_key().as_ref());
            NodeKey(k)
        };
        let b_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(b.public_key().as_ref());
            NodeKey(k)
        };
        let now = now_secs();

        let mut c = Coordinator::with_policy(AuthPolicy::Private {
            genesis_set: vec![g.public_key()],
        });
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);

        // Both register (self-ops, caller == inner key).
        let a_reg = Msg::Register { key: a_key };
        let a_auth = sign_authenticator(
            &a,
            &a_reg.encode(),
            now,
            Some(mint_coord_cap(&g, a_key, now + 3600)),
        );
        assert!(
            c.handle_auth(
                a_src,
                AuthRequest {
                    caller: a_key,
                    inner: a_reg,
                    auth: a_auth
                },
                now
            )
            .is_empty()
        );
        let b_reg = Msg::Register { key: b_key };
        let b_auth = sign_authenticator(
            &b,
            &b_reg.encode(),
            now,
            Some(mint_coord_cap(&g, b_key, now + 3600)),
        );
        assert!(
            c.handle_auth(
                b_src,
                AuthRequest {
                    caller: b_key,
                    inner: b_reg,
                    auth: b_auth
                },
                now
            )
            .is_empty()
        );

        // A looks up B: inner key is B, caller (and PoP signer) is A.
        let lk = Msg::Lookup { key: b_key };
        let lauth = sign_authenticator(
            &a,
            &lk.encode(),
            now,
            Some(mint_coord_cap(&g, a_key, now + 3600)),
        );
        let out = c.handle_auth(
            a_src,
            AuthRequest {
                caller: a_key,
                inner: lk,
                auth: lauth,
            },
            now,
        );
        // A receives B's reflexive and its own PunchSync toward B.
        assert!(out.contains(&(
            a_src,
            Msg::LookupResponse {
                key: b_key,
                reflexive: Some(b_src)
            }
        )));
        assert!(out.contains(&(
            a_src,
            Msg::PunchSync {
                peer: b_key,
                peer_reflexive: b_src
            }
        )));
        // The peer-directed fan-out to B carries A's AUTHENTICATED key, not a
        // reverse-mapped or zero key.
        assert!(out.contains(&(
            b_src,
            Msg::PunchSync {
                peer: a_key,
                peer_reflexive: a_src
            }
        )));
    }

    #[test]
    fn anti_poisoning_rejects_self_op_with_mismatched_caller() {
        // A member cannot register or re-advertise ANOTHER node's key: for a
        // self-op the inner key must equal the authenticated caller, so an
        // AuthRequest whose caller differs from the inner Register key is
        // rejected and the reject counter increments.
        use crate::AuthRequest;
        use crate::auth::{AuthPolicy, mint_coord_cap, now_secs, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let g = ed25519::PrivateKey::from_seed(400);
        let attacker = ed25519::PrivateKey::from_seed(401);
        let attacker_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(attacker.public_key().as_ref());
            NodeKey(k)
        };
        let victim_key = {
            let mut k = [0u8; 32];
            k.copy_from_slice(ed25519::PrivateKey::from_seed(402).public_key().as_ref());
            NodeKey(k)
        };
        let now = now_secs();

        let mut c = Coordinator::with_policy(AuthPolicy::Private {
            genesis_set: vec![g.public_key()],
        });
        let src = addr(1, 1111);
        let before = c.rejects();

        // Attacker (validly admitted for its OWN key) tries to Register the
        // victim's key. The PoP verifies against the caller, but the inner key
        // is the victim's — a self-op mismatch, rejected before dispatch.
        let reg = Msg::Register { key: victim_key };
        let auth = sign_authenticator(
            &attacker,
            &reg.encode(),
            now,
            Some(mint_coord_cap(&g, attacker_key, now + 3600)),
        );
        let out = c.handle_auth(
            src,
            AuthRequest {
                caller: attacker_key,
                inner: reg,
                auth,
            },
            now,
        );
        assert!(out.is_empty());
        assert_eq!(
            c.rejects(),
            before + 1,
            "self-op with mismatched caller is rejected"
        );

        // The victim's key never entered the book: an authenticated self-lookup
        // by the attacker for the victim's key resolves to None (no mapping was
        // poisoned into existence).
        let lk = Msg::Lookup { key: victim_key };
        let lauth = sign_authenticator(
            &attacker,
            &lk.encode(),
            now,
            Some(mint_coord_cap(&g, attacker_key, now + 3600)),
        );
        let out = c.handle_auth(
            src,
            AuthRequest {
                caller: attacker_key,
                inner: lk,
                auth: lauth,
            },
            now,
        );
        assert!(out.iter().any(
            |(_, m)| matches!(m, Msg::LookupResponse { key, reflexive: None } if *key == victim_key)
        ));
    }

    #[test]
    fn legacy_unauthenticated_request_rejected_unless_fully_open() {
        use crate::auth::AuthPolicy;
        let key = NodeKey([1u8; 32]);

        // Fully-open: a bare Register is accepted (mapping created, nothing rejected).
        let mut open = Coordinator::new(); // Open { require_pop: false }
        assert!(
            open.handle_legacy(addr(1, 1111), Msg::Register { key }, 0)
                .is_empty()
        );
        assert_eq!(open.rejects(), 0, "fully-open never rejects");
        let out = open.handle_legacy(addr(2, 2222), Msg::Lookup { key }, 0);
        assert!(
            out.iter().any(|(_, m)| matches!(
                m,
                Msg::LookupResponse {
                    reflexive: Some(_),
                    ..
                }
            )),
            "the bare Register created a mapping under fully-open"
        );

        // require_pop: a bare, unauthenticated Register is dropped and counted.
        let mut gated = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });
        let before = gated.rejects();
        assert!(
            gated
                .handle_legacy(addr(1, 1111), Msg::Register { key }, 0)
                .is_empty()
        );
        assert_eq!(gated.rejects(), before + 1);
        // No mapping was created: a lookup for the same key resolves to None.
        let out = gated.handle_legacy(addr(2, 2222), Msg::Lookup { key }, 0);
        // (The lookup is itself a legacy request, also dropped under require_pop —
        // so it too returns empty; the reject counter is the load-bearing assertion.)
        assert!(out.is_empty());
        assert_eq!(gated.rejects(), before + 2);
    }

    #[test]
    fn expired_registration_lookup_is_none_and_fans_no_punch_sync() {
        let mut c = Coordinator::with_policy_and_ttl(
            crate::auth::AuthPolicy::Open { require_pop: false },
            120,
        );
        let a = NodeKey([0xaa; 32]);
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);
        assert!(
            c.handle_at(a_src, Msg::Register { key: a }, 1_000)
                .is_empty()
        );

        // Within TTL: resolves and fans.
        let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_100);
        assert!(out.contains(&(
            b_src,
            Msg::LookupResponse {
                key: a,
                reflexive: Some(a_src)
            }
        )));
        assert!(
            out.iter()
                .any(|(dst, m)| *dst == a_src && matches!(m, Msg::PunchSync { .. }))
        );

        // Past TTL: honest None, and crucially NO PunchSync toward the dead pinhole.
        let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_121);
        assert_eq!(
            out,
            vec![(
                b_src,
                Msg::LookupResponse {
                    key: a,
                    reflexive: None
                }
            )]
        );
    }

    #[test]
    fn keepalive_readvertise_extends_registration_life() {
        let mut c = Coordinator::with_policy_and_ttl(
            crate::auth::AuthPolicy::Open { require_pop: false },
            120,
        );
        let a = NodeKey([0xaa; 32]);
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);
        assert!(
            c.handle_at(a_src, Msg::Register { key: a }, 1_000)
                .is_empty()
        );
        assert!(
            c.handle_at(a_src, Msg::Readvertise { key: a, nonce: 1 }, 1_100)
                .is_empty()
        );
        assert!(
            c.handle_at(a_src, Msg::Readvertise { key: a, nonce: 2 }, 1_200)
                .is_empty()
        );
        let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_300);
        assert!(
            out.contains(&(
                b_src,
                Msg::LookupResponse {
                    key: a,
                    reflexive: Some(a_src)
                }
            )),
            "keepalive readvertises kept the mapping alive well past the boot TTL"
        );
    }

    #[test]
    fn replayed_authenticated_register_from_another_source_cannot_hijack() {
        // H3 at the authenticated handler: an on-path attacker captures a
        // victim's valid AuthRequest{Register} and replays the IDENTICAL bytes
        // from its OWN socket within the freshness window. The PoP still verifies
        // (it is the victim's real signature), so dispatch reaches
        // observe(victim, attacker_src) — which must NOT repoint the live mapping.
        use crate::AuthRequest;
        use crate::auth::{AuthPolicy, now_secs, sign_authenticator};
        use commonware_cryptography::{Signer as _, ed25519};

        let node = ed25519::PrivateKey::from_seed(555);
        let mut nb = [0u8; 32];
        nb.copy_from_slice(node.public_key().as_ref());
        let victim = NodeKey(nb);
        let now = now_secs();

        let mut c = Coordinator::with_policy(AuthPolicy::Open { require_pop: true });
        let victim_src = addr(1, 4000);
        let attacker_src = addr(9, 6666);
        let b_src = addr(2, 2222);

        // The victim registers from its own source (nonce-0 baseline, live).
        let reg = Msg::Register { key: victim };
        let auth = sign_authenticator(&node, &reg.encode(), now, None);
        let authreq = AuthRequest {
            caller: victim,
            inner: reg,
            auth,
        };
        assert!(c.handle_auth(victim_src, authreq.clone(), now).is_empty());

        // The attacker replays the IDENTICAL captured AuthRequest from its own
        // source. PoP re-verifies, but observe must refuse to repoint the mapping.
        assert!(c.handle_auth(attacker_src, authreq, now).is_empty());

        // A lookup still resolves the victim's ORIGINAL reflexive, and the punch
        // fan-out targets it — never the attacker.
        let out = c.handle_at(b_src, Msg::Lookup { key: victim }, now);
        assert!(
            out.contains(&(
                b_src,
                Msg::LookupResponse {
                    key: victim,
                    reflexive: Some(victim_src)
                }
            )),
            "a replayed register from another source must not hijack the mapping"
        );
        assert!(
            out.iter()
                .any(|(dst, m)| *dst == victim_src && matches!(m, Msg::PunchSync { .. })),
            "the punch fan-out targets the victim's real source"
        );
        assert!(
            !out.iter().any(|(dst, _)| *dst == attacker_src),
            "nothing is directed at the attacker's source"
        );
    }
}
