# Rendezvous Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the coordinator rendezvous plane hold up over real time and real NATs: registrations expire honestly (TTL), clients keep them alive (periodic `Readvertise`), the passive side of a hole-punch answers `PunchSync` even while idle, and each punch retry re-fans the sync — so two live nodes actually complete a punch instead of requiring accidental timing alignment.

**Architecture:** Three layers, bottom-up. (1) `AdvertBook` entries gain a `last_seen` timestamp and a TTL; expired entries resolve to `None` and are replaceable. (2) `Coordinator` threads wall-clock `now` through its handlers. (3) `reachability::NatResolver` becomes a handle to a background *pump task* that owns the `NatClient`: it answers unsolicited `PunchSync` while idle, sends keepalive `Readvertise` on an interval (wall-clock-seeded nonces so reboots supersede stale mappings), and serves `resolve()` requests over a channel with per-try re-`Lookup`. Public signatures of `NatResolver` (`bind`/`reflexive`/`resolve`) are preserved, so `bin/node` does not change.

**Tech Stack:** Rust, tokio (select/mpsc/oneshot/interval), existing crates `crates/system/nat-traversal` and `crates/system/reachability`.

**Context anchor:** This is the rendezvous-side preparation under
`docs/adr/2026-07-07-userspace-overlay-net.mdx` — that ADR's Phase 3 moves the
NAT punch onto the plane's own (WireGuard) socket; everything here is what the
punch machinery needs to be *correct over time* regardless of which socket it
rides. Deliberately out of scope (owned by the ADR / later follow-ups):
publishing the reflexive into `EndpointAdvertisement` (wrong until the socket
is shared), coordinator-relayed intro, coordinator reply signatures.

## Global Constraints

- Branch: `feat/coordinator-rendezvous-hardening` off `origin/dev`; merge target is `dev` (repo rule: all task work targets dev).
- Public API of `reachability::NatResolver` must not change (`bin/node/src/main.rs:4630` calls `NatResolver::bind(me, coords, auth)`, then `.reflexive()`; the orchestrator calls `EndpointResolver::resolve(&mut self, peer, advertised)`).
- `nat_traversal::Coordinator::handle(from, msg)` keeps working for the simnat suite (time-frozen convenience, `now = 0`).
- New wire messages: NONE. Tags 8/9 stay reserved; no tag is added — everything here rides existing `Register`/`Readvertise`/`Lookup`/`PunchSync`/`Punch`.
- Existing constants reused: `COORD_STEP_TIMEOUT = 3s`, `PUNCH_STEP_TIMEOUT = 1s`, `PUNCH_TRIES = 3` (orchestrator.rs:293-300). New: `REGISTRATION_TTL_SECS = 120` (nat-traversal), `RENDEZVOUS_KEEPALIVE = 25s` (reachability; distinct from the existing `KEEPALIVE_SECONDS: u16 = 25` WireGuard constant at orchestrator.rs:75 — do not merge them, they gate different planes).
- Every commit message follows repo style (`feat(nat): …`, `test(nat): …`) and ends with the Claude co-author trailer.
- Gates before merge: `cargo test -p nat-traversal -p reachability`, `cargo build -p node-bin -p coordinator-bin`, `cargo fmt --all -- --check`.

---

### Task 1: AdvertBook TTL

**Files:**
- Modify: `crates/system/nat-traversal/src/advert.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (export `REGISTRATION_TTL_SECS`)

**Interfaces:**
- Produces: `ReflexiveAdvert { reflexive, nonce, last_seen: u64 }`; `AdvertBook::with_ttl(ttl_secs: u64)`; `observe(key, src, now: u64)`, `readvertise(key, src, nonce, now: u64) -> AdvertOutcome`, `current(&self, key, now: u64) -> Option<SocketAddr>`, `key_for_src(&self, src, now: u64) -> Option<NodeKey>`; `pub const REGISTRATION_TTL_SECS: u64 = 120`.
- Consumed by: Task 2 (`Coordinator` threads `now` into every call).

- [x] **Step 1: Write the failing tests** (append to the existing `mod tests` in `advert.rs`; also update every existing test call site in this file to pass `now` — use `0` where time is irrelevant)

```rust
#[test]
fn registration_expires_after_ttl() {
    let key = NodeKey([0x01; 32]);
    let mut book = AdvertBook::with_ttl(120);
    book.observe(key, addr(1, 4000), 1_000);
    assert_eq!(book.current(key, 1_000), Some(addr(1, 4000)));
    assert_eq!(book.current(key, 1_120), Some(addr(1, 4000)), "alive at exactly ttl");
    assert_eq!(book.current(key, 1_121), None, "expired past ttl");
    assert_eq!(book.key_for_src(addr(1, 4000), 1_121), None, "reverse map expires too");
}

#[test]
fn readvertise_refreshes_last_seen() {
    let key = NodeKey([0x02; 32]);
    let mut book = AdvertBook::with_ttl(120);
    book.observe(key, addr(1, 4000), 1_000);
    // keepalive at t=1_100 extends life to 1_220.
    assert_eq!(book.readvertise(key, addr(1, 4000), 1, 1_100), AdvertOutcome::Superseded);
    assert_eq!(book.current(key, 1_200), Some(addr(1, 4000)));
    assert_eq!(book.current(key, 1_221), None);
}

#[test]
fn stale_nonce_does_not_extend_life() {
    // A replayed lower-nonce datagram must not keep a mapping alive: only a
    // fresh (strictly-higher-nonce) readvertise or a baseline observe counts.
    let key = NodeKey([0x03; 32]);
    let mut book = AdvertBook::with_ttl(120);
    book.observe(key, addr(1, 4000), 1_000);
    assert_eq!(book.readvertise(key, addr(1, 4000), 5, 1_010), AdvertOutcome::Superseded);
    assert_eq!(book.readvertise(key, addr(9, 9999), 5, 1_100), AdvertOutcome::Stale);
    assert_eq!(book.current(key, 1_131), None, "life still ends 120s after the LAST accepted advert");
}

#[test]
fn expired_entry_is_replaceable_regardless_of_nonce() {
    // The anti-rollback guard (nonce > 0 blocks a nonce-0 observe) only makes
    // sense for a LIVE mapping. Once expired, the entry is dead — a rebooted
    // node re-registering at the baseline must take the slot back.
    let key = NodeKey([0x04; 32]);
    let mut book = AdvertBook::with_ttl(120);
    book.observe(key, addr(1, 4000), 1_000);
    assert_eq!(book.readvertise(key, addr(1, 4000), 999_999, 1_010), AdvertOutcome::Superseded);
    // Within TTL the high-nonce guard still holds:
    book.observe(key, addr(2, 5000), 1_050);
    assert_eq!(book.current(key, 1_050), Some(addr(1, 4000)));
    // After expiry the fresh register wins:
    book.observe(key, addr(2, 5000), 2_000);
    assert_eq!(book.current(key, 2_000), Some(addr(2, 5000)));
    // ...and a fresh low-nonce readvertise also wins over an expired corpse:
    assert_eq!(book.readvertise(key, addr(3, 6000), 1, 3_000), AdvertOutcome::Superseded);
    assert_eq!(book.current(key, 3_000), Some(addr(3, 6000)));
}

#[test]
fn eviction_prefers_expired_entries() {
    let mut book = AdvertBook::with_ttl(120);
    for i in 0..(MAX_ADVERTS as u64) {
        let mut k = [0u8; 32];
        k[..8].copy_from_slice(&i.to_le_bytes());
        // Promote everyone above the baseline so lowest-nonce eviction alone
        // cannot pick a deterministic victim...
        book.readvertise(NodeKey(k), addr(1, 4000), 10, 1_000);
    }
    // ...except one entry that is EXPIRED (last accepted long ago).
    let mut dead = [0u8; 32];
    dead[..8].copy_from_slice(&3u64.to_le_bytes());
    book.readvertise(NodeKey(dead), addr(1, 4000), 11, 500); // nonce 11 > 10, but stale in time at now=1_000+
    // A fresh key at the cap must evict the EXPIRED entry, not a live one.
    book.observe(NodeKey([0xEE; 32]), addr(2, 5000), 1_000);
    assert_eq!(book.current(NodeKey([0xEE; 32]), 1_000), Some(addr(2, 5000)));
    assert_eq!(book.current(NodeKey(dead), 1_000), None);
}
```

- [x] **Step 2: Run tests to verify they fail**

Run: `cargo test -p nat-traversal advert -- --nocapture`
Expected: compile errors (`observe` takes 2 args, no `with_ttl`) — that is the failure mode for a signature-widening task; the new tests define the contract.

- [x] **Step 3: Implement**

In `advert.rs`: add `pub const REGISTRATION_TTL_SECS: u64 = 120;` with a doc comment (≈5 missed 25s keepalives). Extend the struct and every method:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReflexiveAdvert {
    pub reflexive: SocketAddr,
    pub nonce: u64,
    /// Wall-clock seconds of the last ACCEPTED advert (observe or a
    /// superseding readvertise). A stale-nonce replay never refreshes this.
    pub last_seen: u64,
}

pub struct AdvertBook {
    latest: HashMap<NodeKey, ReflexiveAdvert>,
    ttl: u64,
}

impl Default for AdvertBook {
    fn default() -> Self {
        Self { latest: HashMap::new(), ttl: REGISTRATION_TTL_SECS }
    }
}

impl AdvertBook {
    pub fn with_ttl(ttl: u64) -> Self {
        Self { latest: HashMap::new(), ttl }
    }

    fn expired(&self, advert: &ReflexiveAdvert, now: u64) -> bool {
        now.saturating_sub(advert.last_seen) > self.ttl
    }

    pub fn observe(&mut self, key: NodeKey, src: SocketAddr, now: u64) {
        match self.latest.get(&key) {
            // A LIVE mapping past the boot baseline cannot be rolled back by
            // a (possibly replayed) nonce-0 register. An EXPIRED one is dead
            // weight — the fresh register takes the slot back.
            Some(prev) if prev.nonce > 0 && !self.expired(prev, now) => {}
            _ => {
                self.evict_if_full(&key, now);
                self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce: 0, last_seen: now });
            }
        }
    }

    fn evict_if_full(&mut self, incoming: &NodeKey, now: u64) {
        if self.latest.contains_key(incoming) || self.latest.len() < MAX_ADVERTS {
            return;
        }
        // Prefer reclaiming an expired corpse; fall back to the lowest nonce.
        let victim = self
            .latest
            .iter()
            .find(|(_, a)| self.expired(a, now))
            .map(|(k, _)| *k)
            .or_else(|| self.latest.iter().min_by_key(|(_, a)| a.nonce).map(|(k, _)| *k));
        if let Some(victim) = victim {
            self.latest.remove(&victim);
        }
    }

    pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64, now: u64) -> AdvertOutcome {
        match self.latest.get(&key) {
            Some(prev) if nonce <= prev.nonce && !self.expired(prev, now) => AdvertOutcome::Stale,
            _ => {
                self.evict_if_full(&key, now);
                self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce, last_seen: now });
                AdvertOutcome::Superseded
            }
        }
    }

    pub fn current(&self, key: NodeKey, now: u64) -> Option<SocketAddr> {
        self.latest
            .get(&key)
            .filter(|a| !self.expired(a, now))
            .map(|a| a.reflexive)
    }

    pub fn key_for_src(&self, src: SocketAddr, now: u64) -> Option<NodeKey> {
        self.latest
            .iter()
            .filter(|(_, a)| !self.expired(a, now))
            .find(|(_, a)| a.reflexive == src)
            .map(|(k, _)| *k)
    }
}
```

Update the existing tests in this file mechanically: `observe(k, a)` → `observe(k, a, 0)`, `readvertise(k, a, n)` → `readvertise(k, a, n, 0)`, `current(k)` → `current(k, 0)`, `key_for_src(a)` → `key_for_src(a, 0)`. In `lib.rs` add `REGISTRATION_TTL_SECS` to the `pub use advert::{...}` line.

- [x] **Step 4: Run the crate tests** — `advert` tests green; `coordinator`/`client` still red (they call the old signatures) — that is Task 2's job, so scope this run: `cargo test -p nat-traversal advert`. Expected: PASS.

- [x] **Step 5: Commit** — `git add -A && git commit -m "feat(nat): advert book entries expire — TTL + last_seen, expired slots replaceable"` (+ trailer).

---

### Task 2: Coordinator time-threading + honest expiry

**Files:**
- Modify: `crates/system/nat-traversal/src/coordinator.rs`
- Modify: `crates/system/nat-traversal/src/client.rs` (only `run_coordinator`)

**Interfaces:**
- Produces: `Coordinator::with_policy_and_ttl(policy, ttl_secs)`; `handle_at(&mut self, from, msg, now) -> Vec<(SocketAddr, Msg)>`; `handle_legacy(&mut self, from, msg, now)`; `readvertise(&mut self, key, src, nonce, now)`. `handle(from, msg)` survives as the time-frozen (`now = 0`) convenience used by the simnat suite.
- Consumes: Task 1's `AdvertBook` signatures.

- [x] **Step 1: Write the failing tests** (append to `mod tests` in `coordinator.rs`)

```rust
#[test]
fn expired_registration_lookup_is_none_and_fans_no_punch_sync() {
    let mut c = Coordinator::with_policy_and_ttl(crate::auth::AuthPolicy::Open { require_pop: false }, 120);
    let a = NodeKey([0xaa; 32]);
    let a_src = addr(1, 1111);
    let b_src = addr(2, 2222);
    assert!(c.handle_at(a_src, Msg::Register { key: a }, 1_000).is_empty());

    // Within TTL: resolves and fans.
    let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_100);
    assert!(out.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(a_src) })));
    assert!(out.iter().any(|(dst, m)| *dst == a_src && matches!(m, Msg::PunchSync { .. })));

    // Past TTL: honest None, and crucially NO PunchSync toward the dead pinhole.
    let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_121);
    assert_eq!(out, vec![(b_src, Msg::LookupResponse { key: a, reflexive: None })]);
}

#[test]
fn keepalive_readvertise_extends_registration_life() {
    let mut c = Coordinator::with_policy_and_ttl(crate::auth::AuthPolicy::Open { require_pop: false }, 120);
    let a = NodeKey([0xaa; 32]);
    let a_src = addr(1, 1111);
    let b_src = addr(2, 2222);
    assert!(c.handle_at(a_src, Msg::Register { key: a }, 1_000).is_empty());
    assert!(c.handle_at(a_src, Msg::Readvertise { key: a, nonce: 1 }, 1_100).is_empty());
    assert!(c.handle_at(a_src, Msg::Readvertise { key: a, nonce: 2 }, 1_200).is_empty());
    let out = c.handle_at(b_src, Msg::Lookup { key: a }, 1_300);
    assert!(
        out.contains(&(b_src, Msg::LookupResponse { key: a, reflexive: Some(a_src) })),
        "keepalive readvertises kept the mapping alive well past the boot TTL"
    );
}
```

- [x] **Step 2: Run to verify failure** — `cargo test -p nat-traversal coordinator`
Expected: compile error (`with_policy_and_ttl`/`handle_at` missing).

- [x] **Step 3: Implement**

In `coordinator.rs`:

```rust
/// Construct with an explicit policy AND registration TTL (seconds). The
/// default TTL is `REGISTRATION_TTL_SECS`; tests and short-lived rigs shrink it.
pub fn with_policy_and_ttl(policy: AuthPolicy, ttl_secs: u64) -> Self {
    Self { adverts: AdvertBook::with_ttl(ttl_secs), ..Self::with_policy(policy) }
}

/// Handle one datagram at wall-clock `now` (seconds). The time-threading is
/// what lets registrations EXPIRE: a mapping older than the TTL resolves to
/// `None` and never receives a PunchSync fan-out (its pinhole is long dead).
pub fn handle_at(&mut self, from: SocketAddr, msg: Msg, now: u64) -> Vec<(SocketAddr, Msg)> {
    self.handle_with_caller(from, msg, None, now)
}

/// Time-frozen convenience (`now = 0`) for the deterministic sims/tests,
/// where no wall time passes between registration and lookup.
pub fn handle(&mut self, from: SocketAddr, msg: Msg) -> Vec<(SocketAddr, Msg)> {
    self.handle_with_caller(from, msg, None, 0)
}
```

- `handle_legacy(&mut self, from, msg, now)` gains the `now` parameter and passes it through.
- `handle_auth` passes its existing `now` into `handle_with_caller(from, req.inner, Some(req.caller), now)`.
- `handle_with_caller(..., now: u64)` threads `now` into every book call: `observe(key, from, now)`, `readvertise(key, from, nonce, now)`, `current(key, now)`, `key_for_src(from, now)`.
- The pub `readvertise` helper gains `now` and forwards it.

In `client.rs` `run_coordinator`, the legacy arm becomes `coord.handle_legacy(from, m, now)` (the `now` binding already exists in the loop).

Update existing in-crate call sites mechanically: coordinator tests using `c.readvertise(a, x, n)` → `c.readvertise(a, x, n, 0)`; `punch.rs` and `simnat` keep using `handle` (unchanged semantics at `now = 0`).

- [x] **Step 4: Run the whole crate** — `cargo test -p nat-traversal`. Expected: PASS (all modules now compile against the new signatures).

- [x] **Step 5: Commit** — `git commit -m "feat(nat): coordinator registrations expire — now-threaded handlers, TTL builder, no PunchSync to dead pinholes"`.

---

### Task 3: ClientEvent dispatch on NatClient

**Files:**
- Modify: `crates/system/nat-traversal/src/client.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (export `ClientEvent`)

**Interfaces:**
- Produces: `pub enum ClientEvent { BindResponse { reflexive }, LookupResponse { key, reflexive: Option<SocketAddr> }, PunchSync { peer, peer_reflexive }, Punch { from: NodeKey, src: SocketAddr } }`; `NatClient::send_lookup(&self, peer) -> io::Result<()>`; `NatClient::recv_event(&self) -> io::Result<ClientEvent>`.
- Consumes: nothing new. The existing per-method recv loops stay untouched (their tests still pass); `recv_event` is the single-dispatch alternative Task 4's pump uses exclusively, so no two receivers ever race for one socket.

- [x] **Step 1: Write the failing test** (append to `mod tests` in `client.rs`)

```rust
#[tokio::test]
async fn recv_event_dispatches_lookup_response_and_punch_sync_and_filters_forgeries() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock, crate::auth::AuthPolicy::Open { require_pop: false }));

    let a_key = NodeKey([0xaa; 32]);
    let b_key = NodeKey([0xbb; 32]);
    let a = NatClient::bind(a_key, coord_addr).await.unwrap();
    let b = NatClient::bind(b_key, coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();

    // A forged PunchSync from a non-coordinator must NOT surface as an event.
    let forger = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let a_dst = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), a.local_addr().await.unwrap().port());
    forger
        .send_to(&Msg::PunchSync { peer: b_key, peer_reflexive: addr_of(&forger).await }.encode(), a_dst)
        .await
        .unwrap();

    // B looks A up through the event API: the next coordinator-sourced events
    // on B's socket are the LookupResponse and B's own PunchSync.
    b.send_lookup(a_key).await.unwrap();
    let mut saw_lookup = false;
    let mut saw_sync = false;
    for _ in 0..8 {
        match timeout(Duration::from_secs(2), b.recv_event()).await.expect("event").expect("recv") {
            ClientEvent::LookupResponse { key, reflexive } if key == a_key => {
                assert!(reflexive.is_some());
                saw_lookup = true;
            }
            ClientEvent::PunchSync { peer, .. } if peer == a_key => saw_sync = true,
            _ => {}
        }
        if saw_lookup && saw_sync {
            break;
        }
    }
    assert!(saw_lookup && saw_sync, "lookup response and caller-side punch sync both dispatched");

    // A's socket sees the coordinator's fan-out PunchSync (about B) — and the
    // forged one it received earlier must have been dropped, so the FIRST
    // PunchSync event names B via the coordinator, not the forger's address.
    let ev = timeout(Duration::from_secs(2), a.recv_event()).await.expect("event").expect("recv");
    match ev {
        ClientEvent::PunchSync { peer, peer_reflexive } => {
            assert_eq!(peer, b_key);
            assert_eq!(peer_reflexive.port(), b.local_addr().await.unwrap().port());
        }
        other => panic!("expected the coordinator fan-out PunchSync first, got {other:?}"),
    }
}

async fn addr_of(sock: &UdpSocket) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), sock.local_addr().unwrap().port())
}
```

- [x] **Step 2: Run to verify failure** — `cargo test -p nat-traversal recv_event`
Expected: compile error (`ClientEvent`, `send_lookup`, `recv_event` missing).

- [x] **Step 3: Implement** (in `client.rs`, above the `run_coordinator` free function)

```rust
/// One decoded datagram from the rendezvous socket, classified for a single
/// dispatching consumer. Coordinator-originated control (BindResponse,
/// LookupResponse, PunchSync) is only surfaced when it actually came from the
/// coordinator this client is pointed at — a forged control datagram from
/// anyone else is dropped here, exactly like the per-method recv loops.
/// `Punch` is peer-originated by design, so it carries its observed source
/// for the consumer to match against the rendezvous-resolved address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientEvent {
    BindResponse { reflexive: SocketAddr },
    LookupResponse { key: NodeKey, reflexive: Option<SocketAddr> },
    PunchSync { peer: NodeKey, peer_reflexive: SocketAddr },
    Punch { from: NodeKey, src: SocketAddr },
}

impl NatClient {
    /// Fire-and-forget Lookup — the response arrives as a
    /// [`ClientEvent::LookupResponse`] via [`Self::recv_event`]. The blocking
    /// [`Self::lookup`] stays for sequential callers; a dispatching consumer
    /// (the reachability pump) must NOT mix the two on one socket.
    pub async fn send_lookup(&self, peer: NodeKey) -> std::io::Result<()> {
        self.sock
            .send_to(&self.authed(Msg::Lookup { key: peer }), self.coord)
            .await?;
        Ok(())
    }

    /// Receive the next classified event. Never returns coordinator-shaped
    /// control from a non-coordinator source; undecodable datagrams are
    /// skipped.
    pub async fn recv_event(&self) -> std::io::Result<ClientEvent> {
        let mut buf = [0u8; 128];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            let Ok(msg) = Msg::decode(&buf[..n]) else { continue };
            let from_coord = from == self.coord;
            match msg {
                Msg::BindResponse { reflexive } if from_coord => {
                    return Ok(ClientEvent::BindResponse { reflexive });
                }
                Msg::LookupResponse { key, reflexive } if from_coord => {
                    return Ok(ClientEvent::LookupResponse { key, reflexive });
                }
                Msg::PunchSync { peer, peer_reflexive } if from_coord => {
                    return Ok(ClientEvent::PunchSync { peer, peer_reflexive });
                }
                Msg::Punch { from: peer } => {
                    return Ok(ClientEvent::Punch { from: peer, src: from });
                }
                _ => continue,
            }
        }
    }
}
```

Export in `lib.rs`: `pub use client::{ClientEvent, NatClient, run_coordinator};`

- [x] **Step 4: Run** — `cargo test -p nat-traversal`. Expected: PASS.

- [x] **Step 5: Commit** — `git commit -m "feat(nat): single-dispatch ClientEvent recv on NatClient (+ fire-and-forget send_lookup)"`.

---

### Task 4: NatResolver rendezvous pump (keepalive + idle punch responder + per-try re-Lookup)

**Files:**
- Modify: `crates/system/reachability/src/orchestrator.rs` (the `NatResolver` block, lines ~293-390, and its `EndpointResolver` impl)
- Modify: `crates/system/reachability/src/lib.rs` only if `RENDEZVOUS_KEEPALIVE` should be re-exported (it should: add to the `pub use orchestrator::{...}` list)

**Interfaces:**
- Consumes: Task 3's `ClientEvent`/`recv_event`/`send_lookup`; existing `NatClient::{bind_multi, bind_multi_auth, discover_reflexive_failover, register, readvertise, send_punch_to}`; `nat_traversal::now_secs`.
- Produces (signatures PRESERVED — `bin/node` and the orchestrator body compile untouched):
  - `NatResolver::bind(key, coordinators, auth) -> io::Result<Self>`
  - `NatResolver::reflexive(&self) -> Option<SocketAddr>`
  - `impl EndpointResolver for NatResolver { async fn resolve(&mut self, peer, _advertised) -> Result<Resolution, String> }`
  - New: `pub const RENDEZVOUS_KEEPALIVE: Duration = Duration::from_secs(25);`

- [x] **Step 1: Write the failing tests** (append to the orchestrator test module; check the file tail for the existing `#[cfg(test)] mod` — add a `mod nat_pump` block there)

```rust
#[tokio::test]
async fn passive_resolver_punches_back_while_idle() {
    use tokio::net::UdpSocket;
    // A real coordinator, open policy.
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(nat_traversal::run_coordinator(
        coord_sock,
        nat_traversal::AuthPolicy::Open { require_pop: false },
    ));

    let a_key = binding::node_key(ValidatorIdentity([0xaa; 32]));
    let b_key = binding::node_key(ValidatorIdentity([0xbb; 32]));
    let mut a = NatResolver::bind(a_key, vec![coord_addr], None).await.unwrap();
    let _b = NatResolver::bind(b_key, vec![coord_addr], None).await.unwrap();

    // B NEVER calls resolve. Under the pre-pump code its socket sat deaf
    // outside resolve() windows, the punch went unanswered, and this resolve
    // failed with "hole-punch failed after 3 tries". The pump answers the
    // coordinator's PunchSync fan-out from B's side while B is idle.
    let advertised: SocketAddr = "203.0.113.9:1".parse().unwrap();
    let resolution = a.resolve(b_key, advertised).await.expect("punch completes");
    match resolution {
        Resolution::Punched(_) => {}
        other => panic!("expected a punched path, got {other:?}"),
    }
}

#[tokio::test]
async fn keepalive_readvertises_hold_the_registration_past_the_ttl() {
    use tokio::net::UdpSocket;
    // A coordinator whose registrations expire after 1 second.
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    let coordinator = nat_traversal::Coordinator::with_policy_and_ttl(
        nat_traversal::AuthPolicy::Open { require_pop: false },
        1,
    );
    tokio::spawn(nat_traversal::run_coordinator_with(coord_sock, coordinator));

    // A keeps itself alive on a 300ms keepalive; X registers once and goes silent.
    let a_key = binding::node_key(ValidatorIdentity([0x0a; 32]));
    let x_key = binding::node_key(ValidatorIdentity([0x0f; 32]));
    let _a = NatResolver::bind_with_keepalive(
        a_key,
        vec![coord_addr],
        None,
        std::time::Duration::from_millis(300),
    )
    .await
    .unwrap();
    let x = nat_traversal::NatClient::bind(x_key, coord_addr).await.unwrap();
    x.register().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1_600)).await;

    // A probe client resolves A (kept alive) but not X (expired).
    let probe = nat_traversal::NatClient::bind(
        binding::node_key(ValidatorIdentity([0x01; 32])),
        coord_addr,
    )
    .await
    .unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(2), probe.lookup(a_key))
        .await
        .expect("bounded")
        .expect("keepalives held A's registration");
    let miss = tokio::time::timeout(std::time::Duration::from_secs(1), probe.lookup(x_key)).await;
    assert!(
        miss.is_err() || miss.unwrap().is_err(),
        "X registered once, sent no keepalives, and must have expired"
    );
}
```

This test needs one more nat-traversal seam: `run_coordinator_with(sock, coordinator)` — fold it into this task (it belongs to the test's contract):

```rust
/// `run_coordinator` with a caller-built Coordinator — the seam tests use to
/// run a custom TTL/policy. `run_coordinator` delegates here.
pub async fn run_coordinator_with(sock: UdpSocket, mut coord: Coordinator) { /* body of today's run_coordinator loop */ }

pub async fn run_coordinator(sock: UdpSocket, policy: AuthPolicy) {
    run_coordinator_with(sock, Coordinator::with_policy(policy)).await
}
```

(Export `run_coordinator_with` from `lib.rs`.)

- [x] **Step 2: Run to verify failure** — `cargo test -p reachability nat_pump`
Expected: compile error (`bind_with_keepalive` missing, `run_coordinator_with` missing), and once stubs exist, `passive_resolver_punches_back_while_idle` FAILS with the hole-punch error under the old resolve body.

- [x] **Step 3: Implement the pump** (replace the `NatResolver` struct + impl + `EndpointResolver` impl in orchestrator.rs)

```rust
/// How often the pump re-advertises this node to its coordinators. Must sit
/// well under common NAT UDP mapping timeouts (~30s) — the keepalive holds
/// the pinhole open AND refreshes the coordinator's registration TTL
/// (`nat_traversal::REGISTRATION_TTL_SECS`). Distinct from the WireGuard
/// `KEEPALIVE_SECONDS`: different plane, different socket.
pub const RENDEZVOUS_KEEPALIVE: Duration = Duration::from_secs(25);

/// The production resolver: a handle to the rendezvous PUMP task that owns
/// the `NatClient`. The pump answers unsolicited `PunchSync` fan-outs while
/// this node is otherwise idle (the passive half of somebody else's punch),
/// re-advertises on a keepalive interval, and serves `resolve()` commands.
/// With NO coordinators configured every resolution is `Advertised` and no
/// task is spawned.
pub struct NatResolver {
    commands: Option<tokio::sync::mpsc::Sender<ResolveCmd>>,
    reflexive: Option<SocketAddr>,
}

struct ResolveCmd {
    peer: NodeKey,
    reply: tokio::sync::oneshot::Sender<Result<Resolution, String>>,
}

impl NatResolver {
    pub async fn bind(
        key: NodeKey,
        coordinators: Vec<SocketAddr>,
        auth: Option<(commonware_cryptography::ed25519::PrivateKey, Option<nat_traversal::CoordCap>)>,
    ) -> std::io::Result<Self> {
        Self::bind_with_keepalive(key, coordinators, auth, RENDEZVOUS_KEEPALIVE).await
    }

    /// `bind` with an explicit keepalive interval (tests shrink it).
    pub async fn bind_with_keepalive(
        key: NodeKey,
        coordinators: Vec<SocketAddr>,
        auth: Option<(commonware_cryptography::ed25519::PrivateKey, Option<nat_traversal::CoordCap>)>,
        keepalive: Duration,
    ) -> std::io::Result<Self> {
        if coordinators.is_empty() {
            return Ok(Self { commands: None, reflexive: None });
        }
        let mut client = match auth {
            Some((signer, cap)) => {
                NatClient::bind_multi_auth(key, coordinators, signer, cap).await?
            }
            None => NatClient::bind_multi(key, coordinators).await?,
        };
        let (_idx, reflexive) = client.discover_reflexive_failover(COORD_STEP_TIMEOUT).await?;
        client.register().await?;
        let (tx, rx) = tokio::sync::mpsc::channel(8);
        tokio::spawn(rendezvous_pump(client, rx, keepalive));
        Ok(Self { commands: Some(tx), reflexive: Some(reflexive) })
    }

    pub fn reflexive(&self) -> Option<SocketAddr> {
        self.reflexive
    }
}

impl EndpointResolver for NatResolver {
    async fn resolve(
        &mut self,
        peer: NodeKey,
        _advertised: SocketAddr,
    ) -> Result<Resolution, String> {
        let Some(commands) = &self.commands else {
            return Ok(Resolution::Advertised);
        };
        let (reply, rx) = tokio::sync::oneshot::channel();
        commands
            .send(ResolveCmd { peer, reply })
            .await
            .map_err(|_| "rendezvous pump terminated".to_string())?;
        rx.await.map_err(|_| "rendezvous pump terminated".to_string())?
    }
}

/// The pump body. Single owner of the rendezvous socket's receive side: one
/// dispatch loop, so a PunchSync arriving between resolves is ANSWERED
/// instead of being eaten by whichever blocking recv happened to be polling.
async fn rendezvous_pump(
    client: NatClient,
    mut commands: tokio::sync::mpsc::Receiver<ResolveCmd>,
    keepalive: Duration,
) {
    let mut tick = tokio::time::interval(keepalive);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    tick.tick().await; // interval fires immediately once — consume the arming tick.
    // Readvertise nonces are wall-clock-seeded so a REBOOTED node's first
    // keepalive strictly supersedes every nonce its previous life published —
    // otherwise the coordinator would hold the dead mapping for a full TTL
    // while rejecting the fresh ones as stale.
    let mut nonce = nat_traversal::now_secs();
    loop {
        tokio::select! {
            cmd = commands.recv() => {
                let Some(ResolveCmd { peer, reply }) = cmd else { return };
                let _ = reply.send(do_resolve(&client, peer).await);
            }
            ev = client.recv_event() => match ev {
                Ok(nat_traversal::ClientEvent::PunchSync { peer_reflexive, .. }) => {
                    // The passive half of a peer's rendezvous: open our pinhole
                    // toward the address the coordinator vouched for. Bounded:
                    // one datagram per coordinator-sourced PunchSync.
                    let _ = client.send_punch_to(peer_reflexive).await;
                }
                Ok(_) => {}
                Err(_) => return, // socket gone — the plane owns restart semantics.
            },
            _ = tick.tick() => {
                nonce = nonce.max(nat_traversal::now_secs()) + 1;
                let _ = client.readvertise(nonce).await;
            }
        }
    }
}

/// One resolve: per TRY, a fresh `Lookup` (each one re-fans `PunchSync` to
/// BOTH sides — the retry is what absorbs a lost fan-out datagram or a peer
/// whose pump was busy), then a punch exchange bounded by
/// `PUNCH_STEP_TIMEOUT`. PunchSyncs arriving mid-resolve are answered inline
/// (this node can be the passive side of a DIFFERENT pair's rendezvous at
/// the same time).
async fn do_resolve(client: &NatClient, peer: NodeKey) -> Result<Resolution, String> {
    use nat_traversal::ClientEvent;
    let mut lookup_timeouts = 0usize;
    for _ in 0..PUNCH_TRIES {
        client
            .send_lookup(peer)
            .await
            .map_err(|e| format!("coordinator lookup: {e}"))?;
        let looked_up = tokio::time::timeout(COORD_STEP_TIMEOUT, async {
            loop {
                match client.recv_event().await {
                    Ok(ClientEvent::LookupResponse { key, reflexive }) if key == peer => {
                        return Ok(reflexive);
                    }
                    Ok(ClientEvent::PunchSync { peer_reflexive, .. }) => {
                        let _ = client.send_punch_to(peer_reflexive).await;
                    }
                    Ok(_) => {}
                    Err(e) => return Err(format!("coordinator lookup: {e}")),
                }
            }
        })
        .await;
        let peer_reflexive = match looked_up {
            Err(_elapsed) => {
                lookup_timeouts += 1;
                continue;
            }
            Ok(Err(e)) => return Err(e),
            Ok(Ok(None)) => return Err("peer not registered with coordinator".into()),
            Ok(Ok(Some(addr))) => addr,
        };
        if let Err(e) = client.send_punch_to(peer_reflexive).await {
            return Err(format!("punch send: {e}"));
        }
        let punched = tokio::time::timeout(PUNCH_STEP_TIMEOUT, async {
            loop {
                match client.recv_event().await {
                    Ok(ClientEvent::Punch { src, .. }) if src == peer_reflexive => return Ok(()),
                    Ok(ClientEvent::PunchSync { peer_reflexive: sync_to, .. }) => {
                        let _ = client.send_punch_to(sync_to).await;
                    }
                    Ok(_) => {}
                    Err(e) => return Err(format!("punch recv: {e}")),
                }
            }
        })
        .await;
        match punched {
            Ok(Ok(())) => return Ok(Resolution::Punched(peer_reflexive)),
            Ok(Err(e)) => return Err(e),
            Err(_elapsed) => continue,
        }
    }
    if lookup_timeouts == PUNCH_TRIES {
        return Err("coordinator lookup timed out".to_string());
    }
    Err(format!("hole-punch failed after {PUNCH_TRIES} tries"))
}
```

Notes for the implementer:
- Delete the old `client: Option<NatClient>` field and the old `resolve` body — the pump replaces them wholesale. `NatResolver` intentionally no longer holds the client.
- `NodeKey`/`NatClient` are already imported in this file (check the existing `use` block; extend it with `ClientEvent` if you prefer a bare name over the qualified path).
- The `no coordinators → Advertised` behavior and the `reflexive()` accessor are asserted by existing tests — do not break them.
- Re-export `RENDEZVOUS_KEEPALIVE` from `reachability/src/lib.rs` alongside the other orchestrator exports.

- [x] **Step 4: Run** — `cargo test -p reachability` and `cargo test -p nat-traversal`. Expected: PASS, including the two new tests.

- [x] **Step 5: Commit** — `git commit -m "feat(reachability): rendezvous pump — idle PunchSync responder, keepalive readvertise, per-try re-Lookup"`.

---

### Task 5: Docs + full gates

**Files:**
- Modify: `docs/deploy/coordinator.md` (add a "Registration lifetime" paragraph under "What it is")
- Modify: `docs/deploy/private-cutover-integration-gap.md` (§"What remains": note that rendezvous keepalive/TTL + passive punch response shipped; the socket-sharing + reflexive-advert items stay with the ADR's Phase 3)

- [x] **Step 1: Write the doc deltas**

`coordinator.md`, after the rendezvous/STUN bullet list, add:

```markdown
- **Registration lifetime** — a `register`/`readvertise` mapping expires
  `REGISTRATION_TTL_SECS` (120 s) after the last accepted advert; an expired
  key resolves to `None` and receives no `PunchSync` (its NAT pinhole is long
  dead anyway). Live nodes hold their mapping with a 25 s keepalive
  `Readvertise` (`reachability::RENDEZVOUS_KEEPALIVE`), which doubles as the
  NAT-pinhole keepalive. The book heals itself across coordinator restarts —
  the same keepalives re-register everyone within one interval.
```

`private-cutover-integration-gap.md` §"What remains" item 1: append a sentence noting the rendezvous plane now (a) expires registrations honestly, (b) keepalive-readvertises from the node pump, and (c) answers PunchSync while idle with per-try re-Lookup on the active side — so punch completion no longer depends on both sides resolving simultaneously; what still moves with the userspace-overlay ADR Phase 3 is the punched-pinhole-to-WireGuard-socket alignment and reflexive-bearing adverts.

- [x] **Step 2: Full gates**

Run, at the worktree root:
- `cargo test -p nat-traversal -p reachability` — Expected: PASS
- `cargo build -p node-bin -p coordinator-bin` — Expected: clean build (proves `bin/node` needed zero changes)
- `cargo fmt --all -- --check` — Expected: no diff
- `cargo clippy -p nat-traversal -p reachability --tests` — Expected: no new warnings

- [x] **Step 3: Commit** — `git commit -m "docs(deploy): registration TTL + rendezvous keepalive in the coordinator recipe"`.

- [x] **Step 4: Merge to dev** — push the branch, open a PR based on `dev`, run an adversarial `/code-review` pass on the diff, then merge into `dev` (user-directed).

## Self-Review Notes

- Spec coverage: TTL (Task 1+2), keepalive (Task 4 pump + Task 2 coordinator side), always-on PunchSync responder (Task 4 pump idle arm + inline responders in `do_resolve`), per-try re-Lookup (Task 4 `do_resolve` loop structure). Deferred items documented in the header.
- Type consistency: `handle_at(from, msg, now)` naming used in Tasks 2 and 4's test; `bind_with_keepalive` defined in Task 4 and used in its Step-1 test; `run_coordinator_with` defined and exported in Task 4 (it is that task's test seam).
- Known risk: `passive_resolver_punches_back_while_idle` relies on loopback delivery timing (no NAT) — the punch exchange has 3×(3s+1s) of budget, ample. The keepalive test sleeps ~1.6s wall-clock; bounded and deterministic enough for CI.
