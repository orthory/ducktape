# Slice 2 — Coordinator Ciphertext Relay + Relay Fallback Implementation Plan

> **Historical record (amended 2026-07-06).** The DERP-style coordinator relay
> this plan references (`RelayRequest`/`RelayGrant`, `request_relay`,
> `relay_send`/`relay_recv`, `drive_with_relay_fallback`, the relay-bind
> caveat) was built and subsequently **removed** — the coordinator is
> rendezvous-only and a failed hole-punch is terminal. Do not plan new work
> against the relay API. See the amendment note in
> `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`.

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add the reachability-plane **coordinator ciphertext relay** (a DERP-lite opaque UDP splice) and the **relay-fallback** effect path, so a dial-out-only validator pair that *cannot* hole-punch (symmetric NAT) still reaches each other end-to-end — with the coordinator forwarding opaque bytes it can never decrypt, and the WireGuard peer endpoint pointed at the relay through the seam Slice 0b already left (`apply_tunnel_plan`'s `peer_endpoint_override`).

**Architecture:** Extend the existing `crates/system/nat-traversal` crate. Two new control messages (`RelayRequest`/`RelayGrant`) let a registered node ask the untrusted coordinator to allocate a *per-pair relay session*; each session exposes **two distinct relay UDP ports, one per side** (TURN-like), so raw/opaque WireGuard datagrams route with **no framing** — the arrival port encodes the session+side, the source is learned on the first packet. The splice holds only two `SocketAddr`s per session and forwards payloads verbatim; it is bounded by an idle timeout. Relay fallback is triggered strictly on `PunchError::NotReachable` — hole-punch always runs first. Determinism for CI comes from a new `SimNat::symmetric` mode (per-destination port allocation) that makes hole-punch fail, after which the modeled relay splice achieves **bidirectional opaque delivery** — asserted on the actual bytes, not just NAT-filter state.

**The load-bearing invariant (why this is safe):** There are **two different relays** and this slice keeps them structurally separate:

- **Validator relay** — data-plane, `wireguard-upgrade`'s `relay_candidates` / `DirectDialFailureEvidence`, validator-only, carries consensus authority. **This slice does not touch it.**
- **Coordinator relay** — reachability-plane, new here. A non-validator packet forwarder *below* WireGuard. It never produces a `ValidatorIdentity`, never a `DirectDialFailureEvidence`, never decrypts, holds only `SocketAddr` pairs. Because it is an endpoint below WireGuard (reached via `peer_endpoint_override`) rather than a WireGuard relay peer, the data plane's "relay must be a validator" rule is preserved intact.

A dedicated invariant test (Task 7) asserts that relaying through `peer_endpoint_override` leaves the validated plan's `relay_candidates()` empty — the two relay concepts never couple.

**Reconciliation with the design doc.** `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md` §"Epic decomposition" lists Slice 2 as "Coordinator ciphertext relay + relay-fallback effect path + `DirectDialFailureEvidence` wiring." Per the epic invariant (§6 "two relay concepts, cleanly separated" and §"Trust and threat model"), this plan **deliberately does not wire `DirectDialFailureEvidence`** into the coordinator relay: that evidence is the *validator* relay's trigger and lives in the data plane. The coordinator relay's trigger is the reachability-plane `PunchError::NotReachable`. Wiring `DirectDialFailureEvidence` here would couple the two relays and hand the untrusted coordinator a consensus-authority artifact — exactly what §6 forbids. The signed `DirectDialFailureEvidence` remains the *validator* relay's concern (unchanged in `wireguard-upgrade`) and is out of scope for this slice.

**Tech Stack:** Rust (edition 2024), `tokio` (UDP + `time` + `select!`), the workspace crate conventions. Wire encoding stays hand-rolled fixed-layout bytes (the existing `wire.rs` reader/`take` style). No serde on the hot path. The relay splice is pure and transport-free for the deterministic proof; a `tokio` relay task provides the real runtime.

## Global Constraints

- The coordinator is **untrusted**. The relay adds session/splice state that holds **only** the public `NodeKey` pair (as rendezvous already does) plus `SocketAddr`s — never private/crypto key material, never plaintext, never a decoded payload. The splice forwards opaque bytes byte-for-byte and never calls `Msg::decode` on relay data.
- **No coupling to the validator relay.** Nothing in this slice imports or constructs `wireguard_upgrade::{ValidatorIdentity, DirectDialFailureEvidence, relay_candidates}` into the coordinator relay path. `nat-traversal` gains **no** dependency on validator-identity types.
- **Hole-punch first, relay second.** The relay is only ever built after `PunchError::NotReachable`. A punchable pair must never touch the relay.
- All new wire integers are big-endian, fixed width; every decode is bounds-checked and returns `Result<_, WireError>`. Trailing bytes are rejected (the existing `r.pos != buf.len()` guard).
- No `unwrap()`/`expect()` in library code paths except in tests. Relay state is bounded by an idle timeout in both the pure table (logical ticks) and the real relay task (`tokio` idle timeout).
- The merge gate is scoped to the crates this slice touches (`nat-traversal` + `wireguard-effect`, plus a `coordinator-bin` sanity build). **Do not** pull `bin/node` / `noded` into the gate — their clippy is pre-existingly red from toolchain drift in unrelated dep crates.

---

### Task 1: Relay control messages on the wire (`RelayRequest` / `RelayGrant`)

**Files:**
- Modify: `crates/system/nat-traversal/src/wire.rs` (two `Msg` variants + `u64` codec + tags)
- Modify: `crates/system/nat-traversal/src/coordinator.rs` (keep `handle`'s match exhaustive)
- Test: inline `#[cfg(test)]` in `src/wire.rs`

**Interfaces:**
- Extends `Msg` with:
  - `RelayRequest { peer: NodeKey }` — a registered node asks the coordinator to allocate a relay session to reach `peer`.
  - `RelayGrant { session: u64, relay: SocketAddr }` — coordinator's answer: the shared session id and **this caller's** relay-side socket address (the address it points WireGuard at on fallback).
- Adds `put_u64` / `Reader::u64` big-endian helpers. Tags `TAG_RELAY_REQ = 8`, `TAG_RELAY_GRANT = 9`.

- [ ] **Step 1: Extend the roundtrip test (RED)**

In `crates/system/nat-traversal/src/wire.rs`, add the two new variants to the `every_variant_roundtrips` cases vector (inside the existing `#[cfg(test)] mod tests`):

```rust
            Msg::RelayRequest { peer: NodeKey([11u8; 32]) },
            Msg::RelayGrant { session: 0x0102_0304_0506_0708, relay: addr(12, 51820) },
```

Add a focused test below `decode_rejects_trailing_garbage_bytes`:

```rust
    #[test]
    fn relay_grant_carries_session_and_addr() {
        let m = Msg::RelayGrant { session: 42, relay: addr(3, 4000) };
        let back = Msg::decode(&m.encode()).expect("decode");
        assert_eq!(m, back);
        // Trailing garbage after a RelayGrant is still rejected.
        let mut bytes = m.encode();
        bytes.push(0xff);
        assert_eq!(Msg::decode(&bytes), Err(WireError::Trailing));
    }
```

- [ ] **Step 2: Run it to confirm it fails**

Run: `cargo test -p nat-traversal wire::`
Expected: FAIL — `Msg::RelayRequest` / `Msg::RelayGrant` not defined.

- [ ] **Step 3: Add the variants + tags**

In the `Msg` enum:

```rust
    RelayRequest { peer: NodeKey },
    RelayGrant { session: u64, relay: SocketAddr },
```

Below the existing `TAG_PUNCH`:

```rust
const TAG_RELAY_REQ: u8 = 8;
const TAG_RELAY_GRANT: u8 = 9;
```

- [ ] **Step 4: Add the `u64` codec helpers**

Next to `put_key`/`put_addr`:

```rust
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_be_bytes());
}
```

In `impl<'a> Reader<'a>`, next to `key`/`addr`:

```rust
    fn u64(&mut self) -> Result<u64, WireError> {
        let s = self.take(8)?;
        let mut b = [0u8; 8];
        b.copy_from_slice(s);
        Ok(u64::from_be_bytes(b))
    }
```

- [ ] **Step 5: Add encode + decode arms**

In `encode`'s match, after the `Msg::Punch` arm:

```rust
            Msg::RelayRequest { peer } => {
                out.push(TAG_RELAY_REQ);
                put_key(&mut out, peer);
            }
            Msg::RelayGrant { session, relay } => {
                out.push(TAG_RELAY_GRANT);
                put_u64(&mut out, *session);
                put_addr(&mut out, relay);
            }
```

In `decode`'s match, after the `TAG_PUNCH` arm:

```rust
            TAG_RELAY_REQ => Msg::RelayRequest { peer: r.key()? },
            TAG_RELAY_GRANT => Msg::RelayGrant { session: r.u64()?, relay: r.addr()? },
```

- [ ] **Step 6: Keep `coordinator.rs`'s `handle` match exhaustive**

Adding variants to `Msg` breaks the exhaustive match in `Coordinator::handle`. The coordinator's async loop (Task 6) intercepts `RelayRequest` *before* `handle`, and `RelayGrant` is node-directed, so both are ignored defensively here. Extend the final catch-all arm in `handle`:

```rust
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
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p nat-traversal wire::`
Expected: PASS (`every_variant_roundtrips`, `short_buffer_is_error`, `decode_rejects_trailing_garbage_bytes`, `relay_grant_carries_session_and_addr`).

- [ ] **Step 8: Commit**

```bash
git add crates/system/nat-traversal/src/wire.rs crates/system/nat-traversal/src/coordinator.rs
git commit -m "feat(nat-traversal): RelayRequest/RelayGrant control messages + u64 codec"
```

---

### Task 2: Coordinator relay session table (per-pair allocation + idle prune)

**Files:**
- Modify: `crates/system/nat-traversal/src/coordinator.rs` (session table + `request_relay` + `prune_relays` + `Side`)
- Modify: `crates/system/nat-traversal/src/lib.rs` (re-export `Side`)
- Test: inline `#[cfg(test)]` in `src/coordinator.rs`

**Interfaces:**
- Produces `pub enum Side { A, B }` (which side of the unordered pair a caller is).
- Extends `Coordinator` with a relay session table keyed by the **canonical (sorted) `NodeKey` pair**:
  - `pub fn request_relay(&mut self, caller_src: SocketAddr, peer: NodeKey, now: u64) -> Option<(u64, Side)>` — reverse-maps `caller_src` to its registered key (like `Lookup` does), allocates/reuses the session for `{caller, peer}`, bumps `last_activity`, returns `(session, side)`. `None` if the caller never registered.
  - `pub fn prune_relays(&mut self, now: u64, idle_ticks: u64)` — drops sessions idle longer than `idle_ticks`, keeping relay state bounded.
- Holds only public `NodeKey`s + a `u64` session id + a `u64` logical clock. No key material, no plaintext, no `ValidatorIdentity`.

- [ ] **Step 1: Write the failing tests (RED)**

Add to the `#[cfg(test)] mod tests` in `coordinator.rs`:

```rust
    #[test]
    fn relay_request_allocates_one_session_per_unordered_pair() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let b_src = addr(2, 2222);
        let a = NodeKey([0xaa; 32]);
        let b = NodeKey([0xbb; 32]);
        c.handle(a_src, Msg::Register { key: a });
        c.handle(b_src, Msg::Register { key: b });

        let (s_a, side_a) = c.request_relay(a_src, b, 0).expect("a side");
        let (s_b, side_b) = c.request_relay(b_src, a, 0).expect("b side");
        // Same unordered pair -> one shared session, opposite sides.
        assert_eq!(s_a, s_b);
        assert_ne!(side_a, side_b);
    }

    #[test]
    fn relay_request_without_registration_is_none() {
        let mut c = Coordinator::new();
        let stranger = addr(9, 9999);
        assert!(c.request_relay(stranger, NodeKey([0xbb; 32]), 0).is_none());
    }

    #[test]
    fn prune_relays_drops_idle_sessions() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let a = NodeKey([0xaa; 32]);
        let b = NodeKey([0xbb; 32]);
        c.handle(a_src, Msg::Register { key: a });

        let (s0, _) = c.request_relay(a_src, b, 0).expect("session");
        c.prune_relays(100, 10); // idle 100 > 10 -> gone
        // A fresh request re-allocates a NEW session id (the old one was pruned).
        let (s1, _) = c.request_relay(a_src, b, 200).expect("session");
        assert_ne!(s0, s1);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal coordinator::`
Expected: FAIL — `request_relay` / `prune_relays` / `Side` not defined.

- [ ] **Step 3: Add `Side` + the session type + canonical helper**

At the top of `coordinator.rs` (after the `use` lines):

```rust
/// Which side of an unordered relay pair a caller is. The pair is stored in
/// canonical (byte-sorted) key order; `A` is the smaller key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    A,
    B,
}

struct RelaySession {
    a: NodeKey,
    b: NodeKey,
    last_activity: u64,
}

fn canonical_pair(x: NodeKey, y: NodeKey) -> (NodeKey, NodeKey) {
    if x.0 <= y.0 { (x, y) } else { (y, x) }
}
```

- [ ] **Step 4: Extend the `Coordinator` struct**

```rust
#[derive(Default)]
pub struct Coordinator {
    reflexive: HashMap<NodeKey, SocketAddr>,
    relay_by_pair: HashMap<(NodeKey, NodeKey), u64>,
    relay_sessions: HashMap<u64, RelaySession>,
    next_session: u64,
}
```

- [ ] **Step 5: Add `request_relay` + `prune_relays` to `impl Coordinator`**

```rust
    /// Reachability-plane relay allocation. Given the caller's observed source
    /// and the peer key it wants to relay to, return the shared session id for
    /// the unordered `{caller, peer}` pair and which side the caller is. The
    /// caller must have registered (so its source can be bound to its key),
    /// else `None`.
    ///
    /// This is deliberately NOT the wireguard-upgrade validator relay: it never
    /// produces a `ValidatorIdentity` or a `DirectDialFailureEvidence` and
    /// carries no consensus authority. It holds only public `NodeKey`s and a
    /// session id.
    pub fn request_relay(
        &mut self,
        caller_src: SocketAddr,
        peer: NodeKey,
        now: u64,
    ) -> Option<(u64, Side)> {
        let caller = self
            .reflexive
            .iter()
            .find(|&(_, &v)| v == caller_src)
            .map(|(k, _)| *k)?;
        let (a, b) = canonical_pair(caller, peer);
        let session = match self.relay_by_pair.get(&(a, b)) {
            Some(&s) => s,
            None => {
                let s = self.next_session;
                self.next_session = self.next_session.wrapping_add(1);
                self.relay_by_pair.insert((a, b), s);
                s
            }
        };
        let entry = self
            .relay_sessions
            .entry(session)
            .or_insert(RelaySession { a, b, last_activity: now });
        entry.last_activity = now;
        let side = if caller == a { Side::A } else { Side::B };
        Some((session, side))
    }

    /// Drop relay sessions idle longer than `idle_ticks`. Keeps relay state
    /// bounded: a session with no traffic is torn down so the coordinator never
    /// accumulates unbounded `SocketAddr` pairs.
    pub fn prune_relays(&mut self, now: u64, idle_ticks: u64) {
        let expired: Vec<u64> = self
            .relay_sessions
            .iter()
            .filter(|(_, s)| now.saturating_sub(s.last_activity) > idle_ticks)
            .map(|(&id, _)| id)
            .collect();
        for id in expired {
            if let Some(s) = self.relay_sessions.remove(&id) {
                self.relay_by_pair.remove(&(s.a, s.b));
            }
        }
    }
```

- [ ] **Step 6: Re-export `Side` from `lib.rs`**

Change the coordinator re-export line in `lib.rs`:

```rust
pub use coordinator::{Coordinator, Side};
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p nat-traversal coordinator::`
Expected: PASS (existing 3 + new 3 = 6 tests).

- [ ] **Step 8: Commit**

```bash
git add crates/system/nat-traversal/src/coordinator.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): per-pair relay session table with idle prune"
```

---

### Task 3: `RelaySplice` — the pure opaque forwarding model

**Files:**
- Create: `crates/system/nat-traversal/src/relay.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (`pub mod relay;` + re-exports)
- Test: inline `#[cfg(test)]` in `src/relay.rs`

**Interfaces:**
- `pub struct RelaySplice` — one session's two-port splice. Holds only two egress `SocketAddr`s + the two learned source `SocketAddr`s + a `last_activity` tick. **Never** a key, **never** plaintext.
- `pub struct Forward { pub from: SocketAddr, pub to: SocketAddr, pub payload: Vec<u8> }` — a datagram to emit: `from` is the relay egress port (the other side's port), `to` is the learned destination, `payload` is the opaque bytes forwarded verbatim.
- `impl RelaySplice`:
  - `pub fn new(a_egress: SocketAddr, b_egress: SocketAddr, now: u64) -> Self`
  - `pub fn ingress(&mut self, side: Side, src: SocketAddr, now: u64, payload: Vec<u8>) -> Option<Forward>` — learn-on-first: record `src` for `side`; if the *other* side's source is known, return the `Forward` toward it; else `None` (nowhere to forward yet).
  - `pub fn is_idle(&self, now: u64, idle_ticks: u64) -> bool`

- [ ] **Step 1: Write the failing tests (RED)**

`crates/system/nat-traversal/src/relay.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::Side;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, o)), p)
    }

    fn relay(port: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), port)
    }

    #[test]
    fn buffers_until_both_sides_have_sent_then_forwards_verbatim() {
        let a_egress = relay(4000);
        let b_egress = relay(4001);
        let mut s = RelaySplice::new(a_egress, b_egress, 0);
        let a_src = addr(1, 5000);
        let b_src = addr(2, 6000);

        // A's first datagram: B's source not yet known -> nowhere to forward.
        assert_eq!(s.ingress(Side::A, a_src, 1, b"opaque-A".to_vec()), None);

        // B sends: now A's source is known -> forward B's payload to A via a_egress.
        let to_a = s.ingress(Side::B, b_src, 2, b"opaque-B".to_vec()).expect("forward");
        assert_eq!(to_a, Forward { from: a_egress, to: a_src, payload: b"opaque-B".to_vec() });

        // A sends again: B's source now known -> forward A's payload to B via b_egress.
        let to_b = s.ingress(Side::A, a_src, 3, b"opaque-A".to_vec()).expect("forward");
        assert_eq!(to_b, Forward { from: b_egress, to: b_src, payload: b"opaque-A".to_vec() });
    }

    #[test]
    fn payload_is_forwarded_byte_for_byte_never_interpreted() {
        let mut s = RelaySplice::new(relay(4000), relay(4001), 0);
        // A control-message-looking byte sequence must be forwarded verbatim,
        // never decoded: the relay is opaque.
        let looks_like_control = vec![3u8, 0, 0, 0]; // TAG_REGISTER prefix + junk
        s.ingress(Side::A, addr(1, 5000), 1, looks_like_control.clone());
        let f = s.ingress(Side::B, addr(2, 6000), 2, vec![7, 7, 7]).expect("forward");
        // B->A carried [7,7,7] untouched; the A payload is likewise untouched
        // when re-driven.
        assert_eq!(f.payload, vec![7, 7, 7]);
        let f2 = s.ingress(Side::A, addr(1, 5000), 3, looks_like_control.clone()).expect("forward");
        assert_eq!(f2.payload, looks_like_control);
    }

    #[test]
    fn is_idle_after_timeout() {
        let mut s = RelaySplice::new(relay(4000), relay(4001), 0);
        s.ingress(Side::A, addr(1, 5000), 10, b"x".to_vec());
        assert!(!s.is_idle(15, 10));
        assert!(s.is_idle(30, 10)); // 30 - 10 > 10
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal relay::`
Expected: FAIL — `RelaySplice` / `Forward` not defined.

- [ ] **Step 3: Implement `relay.rs`**

```rust
use std::net::SocketAddr;

use crate::Side;

/// One relay session's opaque UDP splice. The coordinator relay learns each
/// side's source address on that side's first datagram (learn-on-first), then
/// forwards every subsequent OPAQUE datagram to the other side verbatim. It
/// holds only the two learned `SocketAddr`s and the two egress addresses —
/// never a key, never plaintext, never the datagram's meaning. Bounded by an
/// idle timeout via `last_activity`.
pub struct RelaySplice {
    a_egress: SocketAddr,
    b_egress: SocketAddr,
    a_src: Option<SocketAddr>,
    b_src: Option<SocketAddr>,
    last_activity: u64,
}

/// A datagram the splice wants to emit. `from` is the relay egress port the
/// datagram leaves from (the other side's port); `to` is the learned
/// destination; `payload` is forwarded byte-for-byte.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Forward {
    pub from: SocketAddr,
    pub to: SocketAddr,
    pub payload: Vec<u8>,
}

impl RelaySplice {
    pub fn new(a_egress: SocketAddr, b_egress: SocketAddr, now: u64) -> Self {
        Self {
            a_egress,
            b_egress,
            a_src: None,
            b_src: None,
            last_activity: now,
        }
    }

    /// A datagram arrived on `side`'s relay socket from `src` carrying opaque
    /// `payload`. Record the source (learn-on-first) and, if the other side's
    /// source is known, return the `Forward` to emit toward it. Until the other
    /// side has sent at least once there is nowhere to forward, so returns
    /// `None` (the datagram is dropped — real WireGuard retransmits).
    pub fn ingress(
        &mut self,
        side: Side,
        src: SocketAddr,
        now: u64,
        payload: Vec<u8>,
    ) -> Option<Forward> {
        self.last_activity = now;
        match side {
            Side::A => {
                self.a_src = Some(src);
                self.b_src.map(|to| Forward {
                    from: self.b_egress,
                    to,
                    payload,
                })
            }
            Side::B => {
                self.b_src = Some(src);
                self.a_src.map(|to| Forward {
                    from: self.a_egress,
                    to,
                    payload,
                })
            }
        }
    }

    pub fn is_idle(&self, now: u64, idle_ticks: u64) -> bool {
        now.saturating_sub(self.last_activity) > idle_ticks
    }
}
```

- [ ] **Step 4: Wire the module into `lib.rs`**

`relay` is pure (no `tokio`, no `simnat`), so it is always compiled. Add:

```rust
pub mod relay;
```

and extend the re-exports:

```rust
pub use relay::{Forward, RelaySplice};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal relay::`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/relay.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): opaque RelaySplice (learn-on-first, idle-bounded)"
```

---

### Task 4: `SimNat::symmetric` — model the pair that cannot hole-punch

**Files:**
- Modify: `crates/system/nat-traversal/src/simnat.rs` (add a symmetric mapping mode)
- Modify: `crates/system/nat-traversal/src/punch.rs` (a test locking `NotReachable`)
- Test: inline `#[cfg(test)]` in both files

**Interfaces:**
- Adds `pub fn SimNat::symmetric(public_ip: IpAddr) -> Self` — a **symmetric NAT**: a fresh public port per `(internal, dst)` pair, so the reflexive port the coordinator observed (mapping toward the coordinator) is never the port that would admit a peer's punch (mapping toward the peer differs). `SimNat::new` is unchanged (restricted-cone, endpoint-independent). `allow_inbound` semantics are unchanged.
- Consequence: `drive_simulated` on a symmetric pair returns `Err(PunchError::NotReachable)` — the fallback trigger.

- [ ] **Step 1: Write the failing tests (RED)**

Add to `simnat.rs`'s `#[cfg(test)] mod tests`:

```rust
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
```

Add to `punch.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn symmetric_nat_pair_fails_hole_punch_with_not_reachable() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let err = drive_simulated(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord)
            .expect_err("symmetric NAT must defeat hole-punch");
        assert_eq!(err, PunchError::NotReachable);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal --features simnat`
Expected: FAIL — `SimNat::symmetric` not defined.

- [ ] **Step 3: Add the mapping mode to `simnat.rs`**

Replace the struct + `new` + `send` with a mode-aware version (keeping `SimNat::new`'s restricted-cone behavior byte-for-byte identical so Slice 0a's tests stay green):

```rust
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat`
Expected: PASS — the two new tests plus every existing `simnat::` / `punch::` test (restricted-cone behavior unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/simnat.rs crates/system/nat-traversal/src/punch.rs
git commit -m "feat(nat-traversal): SimNat::symmetric — the pair hole-punch cannot beat"
```

---

### Task 5: Relay fallback over `SimNat` — the deterministic merge-gate proof

**Files:**
- Modify: `crates/system/nat-traversal/src/punch.rs` (`drive_with_relay_fallback` + `FallbackOutcome` + `RelayFallbackProof`)
- Modify: `crates/system/nat-traversal/src/lib.rs` (re-exports under the `simnat` gate)
- Test: inline `#[cfg(test)]` in `src/punch.rs`

**Interfaces:**
- `pub enum FallbackOutcome { Punched { a: PunchPlan, b: PunchPlan }, Relayed(RelayFallbackProof) }` — encodes "hole-punch first, relay only on failure" in the type.
- `pub struct RelayFallbackProof { pub a_relay_endpoint: SocketAddr, pub b_relay_endpoint: SocketAddr, pub delivered_to_b: Vec<u8>, pub delivered_to_a: Vec<u8> }` — the relay endpoint each side points WireGuard at (`peer_endpoint_override` on the fallback path), plus the opaque bytes **actually delivered** end-to-end.
- `pub fn drive_with_relay_fallback(a_key, b_key, a_nat, b_nat, coord, a_payload, b_payload) -> Result<FallbackOutcome, PunchError>` — attempts `drive_simulated` first; on `Ok` returns `Punched`; on `NotReachable` allocates the relay session, splices the two payloads through both NATs, asserts real bidirectional delivery, and returns `Relayed`.

- [ ] **Step 1: Write the failing merge-gate tests (RED)**

Add to `punch.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn symmetric_pair_falls_back_to_relay_and_delivers_bidirectionally() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let outcome = drive_with_relay_fallback(
            a_key,
            b_key,
            &mut a_nat,
            &mut b_nat,
            &mut coord,
            b"ping-from-a",
            b"pong-from-b",
        )
        .expect("relay fallback");

        match outcome {
            FallbackOutcome::Relayed(p) => {
                // ACTUAL delivery of the opaque bytes, not just NAT-filter state.
                assert_eq!(p.delivered_to_b, b"ping-from-a");
                assert_eq!(p.delivered_to_a, b"pong-from-b");
                // Two distinct relay ports, one per side.
                assert_ne!(p.a_relay_endpoint, p.b_relay_endpoint);
            }
            FallbackOutcome::Punched { .. } => panic!("symmetric pair must NOT hole-punch"),
        }
    }

    #[test]
    fn cone_pair_punches_and_never_touches_the_relay() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let outcome = drive_with_relay_fallback(
            a_key, b_key, &mut a_nat, &mut b_nat, &mut coord, b"a", b"b",
        )
        .expect("punch");

        match outcome {
            FallbackOutcome::Punched { a, b } => {
                assert_eq!(a.peer_reflexive, b.local_mapped);
                assert_eq!(b.peer_reflexive, a.local_mapped);
            }
            FallbackOutcome::Relayed(_) => panic!("a punchable pair must not use the relay"),
        }
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: FAIL — `drive_with_relay_fallback` / `FallbackOutcome` / `RelayFallbackProof` not defined.

- [ ] **Step 3: Implement the fallback driver in `punch.rs`**

Update the imports at the top of `punch.rs`:

```rust
use crate::{Coordinator, Msg, NodeKey, Side, relay::RelaySplice, simnat::SimNat};
```

Add below `drive_simulated`:

```rust
/// The proof a relay fallback produces: the relay endpoint each side must point
/// its WireGuard peer at (`apply_tunnel_plan`'s `peer_endpoint_override` on the
/// fallback path), plus the opaque bytes actually delivered end to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayFallbackProof {
    pub a_relay_endpoint: SocketAddr,
    pub b_relay_endpoint: SocketAddr,
    pub delivered_to_b: Vec<u8>,
    pub delivered_to_a: Vec<u8>,
}

/// Outcome of the reachability dance: a direct hole-punched path, or — only
/// when hole-punch failed — the coordinator relay fallback.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FallbackOutcome {
    Punched { a: PunchPlan, b: PunchPlan },
    Relayed(RelayFallbackProof),
}

// The two relay egress ports the coordinator would bind for a session in the
// deterministic model. The real coordinator binds ephemeral ports and reports
// the actual addresses in the `RelayGrant` (Task 6); here they are derived
// from the session id so the model stays reproducible.
fn relay_side_addrs(session: u64) -> (SocketAddr, SocketAddr) {
    use std::net::{IpAddr, Ipv4Addr};
    let ip = IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1));
    let base = 4000u16.wrapping_add((session as u16).wrapping_mul(2));
    (
        SocketAddr::new(ip, base),
        SocketAddr::new(ip, base.wrapping_add(1)),
    )
}

/// Attempt hole-punch FIRST; only on `PunchError::NotReachable` fall back to
/// the coordinator ciphertext relay and prove OPAQUE bidirectional delivery
/// through both NATs. This is the CI proof that a symmetric-NAT pair still
/// reaches each other with neither exposing an inbound port.
pub fn drive_with_relay_fallback(
    a_key: NodeKey,
    b_key: NodeKey,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
    coord: &mut Coordinator,
    a_payload: &[u8],
    b_payload: &[u8],
) -> Result<FallbackOutcome, PunchError> {
    // Hole-punch first. `drive_simulated` registers both nodes with the
    // coordinator as a side effect, which is exactly what relay allocation
    // needs (source->key binding).
    match drive_simulated(a_key, b_key, a_nat, b_nat, coord) {
        Ok((a, b)) => return Ok(FallbackOutcome::Punched { a, b }),
        Err(PunchError::NotReachable) => {}
        Err(e) => return Err(e),
    }

    // The coordinator-facing mapping is idempotent (cone: stable; symmetric:
    // same (internal, coord) key), so re-deriving each mapped source is safe
    // and matches what registration observed.
    let a_mapped = a_nat.send(internal(&a_key), coord_addr());
    let b_mapped = b_nat.send(internal(&b_key), coord_addr());

    // Control plane: both sides request the relay for the same unordered pair.
    let (session, _a_side) = coord
        .request_relay(a_mapped, b_key, 0)
        .ok_or(PunchError::NoReflexive)?;
    let (session_b, _b_side) = coord
        .request_relay(b_mapped, a_key, 0)
        .ok_or(PunchError::NoReflexive)?;
    debug_assert_eq!(session, session_b, "one session per unordered pair");
    let (a_relay_endpoint, b_relay_endpoint) = relay_side_addrs(session);

    // Data plane: each side sends its opaque payload OUT to its (fixed) relay
    // endpoint. Because the destination is stable, even a symmetric NAT opens a
    // durable hole toward the relay, so return traffic from the relay is
    // admitted — this is why the relay beats symmetric NAT.
    let a_mapped_relay = a_nat.send(internal(&a_key), a_relay_endpoint);
    let b_mapped_relay = b_nat.send(internal(&b_key), b_relay_endpoint);

    let mut splice = RelaySplice::new(a_relay_endpoint, b_relay_endpoint, 0);
    // A sends first: B's source not yet learned, so it is buffered/dropped.
    let _ = splice.ingress(Side::A, a_mapped_relay, 1, a_payload.to_vec());
    // B sends: A's source known -> forward B's payload toward A via a_egress.
    let to_a = splice
        .ingress(Side::B, b_mapped_relay, 2, b_payload.to_vec())
        .ok_or(PunchError::NotReachable)?;
    // A re-sends (real WireGuard retransmits): now forward A's payload to B.
    let to_b = splice
        .ingress(Side::A, a_mapped_relay, 3, a_payload.to_vec())
        .ok_or(PunchError::NotReachable)?;

    // Assert ACTUAL delivery: each NAT must admit the relay's egress datagram.
    if !b_nat.allow_inbound(b_mapped_relay, to_b.from)
        || !a_nat.allow_inbound(a_mapped_relay, to_a.from)
    {
        return Err(PunchError::NotReachable);
    }

    Ok(FallbackOutcome::Relayed(RelayFallbackProof {
        a_relay_endpoint,
        b_relay_endpoint,
        delivered_to_b: to_b.payload,
        delivered_to_a: to_a.payload,
    }))
}
```

- [ ] **Step 4: Re-export under the `simnat` gate in `lib.rs`**

Extend the existing `#[cfg(any(test, feature = "simnat"))] pub use punch::...` line:

```rust
#[cfg(any(test, feature = "simnat"))]
pub use punch::{
    FallbackOutcome, PunchError, PunchPlan, RelayFallbackProof, drive_simulated,
    drive_with_relay_fallback,
};
```

(If `drive_simulated` was not previously re-exported, add it here — the Task 7 integration test in `wireguard-effect` consumes `drive_with_relay_fallback` from the crate root.)

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: PASS — both new tests plus the existing `punch::` suite.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/punch.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): drive_with_relay_fallback — punch-first, relay-on-failure, asserted delivery"
```

---

### Task 6: Real async relay serving (`run_relay_pair` + `run_coordinator` relay path + `NatClient` helpers)

**Files:**
- Modify: `crates/system/nat-traversal/src/client.rs` (`run_relay_pair`, extend `run_coordinator`, `NatClient` relay helpers)
- Modify: `crates/system/nat-traversal/src/lib.rs` (re-export `run_relay_pair`)
- Test: inline `#[cfg(test)]` async tests in `src/client.rs`

**Interfaces:**
- `pub async fn run_relay_pair(a_sock: UdpSocket, b_sock: UdpSocket, idle: Duration)` — the real opaque splice for one session: `select!` over both sockets, learn each side's source on its first datagram, forward opaque bytes (1500-byte MTU buffers) to the other side's learned source, and **tear down after `idle` of total inactivity**. Never decodes payloads.
- `run_coordinator` gains a `RelayRequest` arm: reverse-map the caller, `coord.request_relay`, bind two ephemeral relay sockets on the coordinator's own IP on first allocation, spawn a `run_relay_pair`, and answer with `RelayGrant { session, relay: <this side's addr> }`.
- `NatClient` gains: `request_relay(&self, peer) -> io::Result<(u64, SocketAddr)>`, `relay_send(&self, relay, payload)`, `relay_recv(&self) -> io::Result<Vec<u8>>`.

- [ ] **Step 1: Write the failing tests (RED)**

Add to `client.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn two_clients_relay_opaque_datagrams_both_ways() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        let (s_a, a_relay) = timeout(Duration::from_secs(2), a.request_relay(b_key))
            .await
            .expect("no timeout")
            .expect("grant a");
        let (s_b, b_relay) = timeout(Duration::from_secs(2), b.request_relay(a_key))
            .await
            .expect("no timeout")
            .expect("grant b");
        assert_eq!(s_a, s_b, "one session per pair");
        assert_ne!(a_relay, b_relay, "one relay port per side");

        // The relay learns a side's source on its first datagram, so the far
        // side's source must already be known before a payload can be
        // forwarded (real WireGuard retransmits). Sequence the sends: B first
        // (learned, dropped), then A (A->B delivered), then B again (B->A).
        b.relay_send(b_relay, b"drop-until-a-known").await.unwrap();
        a.relay_send(a_relay, b"opaque-ciphertext-A").await.unwrap();
        let got_b = timeout(Duration::from_secs(2), b.relay_recv())
            .await
            .expect("no timeout")
            .expect("recv b");
        assert_eq!(got_b, b"opaque-ciphertext-A");

        b.relay_send(b_relay, b"opaque-ciphertext-B").await.unwrap();
        let got_a = timeout(Duration::from_secs(2), a.relay_recv())
            .await
            .expect("no timeout")
            .expect("recv a");
        assert_eq!(got_a, b"opaque-ciphertext-B");
    }

    #[tokio::test]
    async fn relay_pair_tears_down_after_idle() {
        let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let handle = tokio::spawn(run_relay_pair(a, b, Duration::from_millis(50)));
        // No traffic on either side -> the task returns within a bounded time.
        timeout(Duration::from_secs(1), handle)
            .await
            .expect("relay pair did not idle out")
            .expect("join");
    }
```

Ensure the test module imports cover `Duration`/`timeout` (the file already uses `tokio::time::{Duration, timeout}` in an existing test).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal client::`
Expected: FAIL — `request_relay` / `relay_send` / `relay_recv` / `run_relay_pair` not defined.

- [ ] **Step 3: Add the `NatClient` relay helpers**

In `impl NatClient` (in `client.rs`):

```rust
    /// Ask the coordinator to allocate a relay session to `peer`; return the
    /// session id and THIS side's relay endpoint — the address to point the
    /// WireGuard peer at on hole-punch failure (`peer_endpoint_override`).
    pub async fn request_relay(&self, peer: NodeKey) -> std::io::Result<(u64, SocketAddr)> {
        self.sock
            .send_to(&Msg::RelayRequest { peer }.encode(), self.coord)
            .await?;
        let mut buf = [0u8; 64];
        loop {
            let (n, from) = self.sock.recv_from(&mut buf).await?;
            if from != self.coord {
                continue;
            }
            if let Ok(Msg::RelayGrant { session, relay }) = Msg::decode(&buf[..n]) {
                return Ok((session, relay));
            }
        }
    }

    /// Send an OPAQUE datagram to a relay endpoint. The relay forwards it
    /// verbatim; the bytes are never interpreted by this crate.
    pub async fn relay_send(&self, relay: SocketAddr, payload: &[u8]) -> std::io::Result<()> {
        self.sock.send_to(payload, relay).await?;
        Ok(())
    }

    /// Receive a relayed OPAQUE datagram (up to one MTU). Returns the raw bytes
    /// as delivered by the relay — no decode.
    pub async fn relay_recv(&self) -> std::io::Result<Vec<u8>> {
        let mut buf = [0u8; 1500];
        let (n, _from) = self.sock.recv_from(&mut buf).await?;
        Ok(buf[..n].to_vec())
    }
```

- [ ] **Step 4: Add `run_relay_pair`**

Below `run_coordinator` in `client.rs`:

```rust
/// The real opaque splice for one relay session: forward datagrams between two
/// UDP sockets (one per side), learning each side's source on its first
/// datagram, and tear down after `idle` of total inactivity. Never decodes a
/// payload — it holds only the two learned source addresses.
pub async fn run_relay_pair(a_sock: UdpSocket, b_sock: UdpSocket, idle: std::time::Duration) {
    let mut a_src: Option<SocketAddr> = None;
    let mut b_src: Option<SocketAddr> = None;
    let mut a_buf = [0u8; 1500];
    let mut b_buf = [0u8; 1500];
    loop {
        tokio::select! {
            r = a_sock.recv_from(&mut a_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                a_src = Some(from);
                if let Some(dst) = b_src {
                    let _ = b_sock.send_to(&a_buf[..n], dst).await;
                }
            }
            r = b_sock.recv_from(&mut b_buf) => {
                let (n, from) = match r { Ok(v) => v, Err(_) => continue };
                b_src = Some(from);
                if let Some(dst) = a_src {
                    let _ = a_sock.send_to(&b_buf[..n], dst).await;
                }
            }
            _ = tokio::time::sleep(idle) => {
                // Idle timeout: no datagram on either side within `idle`. The
                // sleep re-arms every loop iteration, so this fires only after
                // `idle` of continuous inactivity. Bounded teardown.
                return;
            }
        }
    }
}
```

- [ ] **Step 5: Extend `run_coordinator` with the relay path**

Replace `run_coordinator` with the relay-aware version:

```rust
/// The coordinator event loop: decode control datagrams, feed the pure handler,
/// send replies. `RelayRequest` is handled specially — it must bind real relay
/// sockets, which the transport-free `Coordinator::handle` cannot do.
pub async fn run_coordinator(sock: UdpSocket) {
    use std::collections::HashMap;

    let mut coord = Coordinator::new();
    let mut buf = [0u8; 64];
    let bind_ip = sock
        .local_addr()
        .map(|a| a.ip())
        .unwrap_or_else(|_| std::net::IpAddr::from([0, 0, 0, 0]));
    // session id -> (side-A relay addr, side-B relay addr)
    let mut relay_addrs: HashMap<u64, (SocketAddr, SocketAddr)> = HashMap::new();
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg = match Msg::decode(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if let Msg::RelayRequest { peer } = msg {
            if let Some((session, side)) = coord.request_relay(from, peer, 0) {
                let pair = match relay_addrs.get(&session) {
                    Some(&pair) => pair,
                    None => {
                        // Bind two ephemeral relay sockets on the coordinator's
                        // own IP and spawn the opaque splice for this session.
                        let a = match UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let b = match UdpSocket::bind(SocketAddr::new(bind_ip, 0)).await {
                            Ok(s) => s,
                            Err(_) => continue,
                        };
                        let (a_addr, b_addr) = match (a.local_addr(), b.local_addr()) {
                            (Ok(x), Ok(y)) => (x, y),
                            _ => continue,
                        };
                        tokio::spawn(run_relay_pair(a, b, std::time::Duration::from_secs(30)));
                        relay_addrs.insert(session, (a_addr, b_addr));
                        (a_addr, b_addr)
                    }
                };
                let relay = match side {
                    Side::A => pair.0,
                    Side::B => pair.1,
                };
                let _ = sock
                    .send_to(&Msg::RelayGrant { session, relay }.encode(), from)
                    .await;
            }
            continue;
        }
        for (dst, reply) in coord.handle(from, msg) {
            let _ = sock.send_to(&reply.encode(), dst).await;
        }
    }
}
```

Update the `use` at the top of `client.rs` to bring in `Side`:

```rust
use crate::{Coordinator, Msg, NodeKey, Side};
```

- [ ] **Step 6: Re-export `run_relay_pair` from `lib.rs`**

```rust
pub use client::{NatClient, run_coordinator, run_relay_pair};
```

- [ ] **Step 7: Run to verify pass**

Run: `cargo test -p nat-traversal client::`
Expected: PASS — the two new async tests plus the existing `client::` suite.

- [ ] **Step 8: Confirm `bin/coordinator` still builds (it inherits the relay for free)**

`run_coordinator`'s signature is unchanged, so `bin/coordinator/src/main.rs` needs no edit; it now serves the relay automatically.

Run: `cargo test -p coordinator-bin && cargo build -p coordinator-bin`
Expected: PASS (existing smoke test) + binary builds.

- [ ] **Step 9: Commit**

```bash
git add crates/system/nat-traversal/src/client.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): real coordinator relay serving + NatClient relay helpers"
```

---

### Task 7: Relay-fallback effect path + the two-relay separation invariant (`wireguard-effect`)

**Files:**
- Modify: `crates/system/wireguard-effect/Cargo.toml` (dev-dep on `nat-traversal` with `simnat`)
- Create: `crates/system/wireguard-effect/tests/relay_fallback.rs`
- Test: the new integration test file

**Interfaces:**
- No new production API. This task **wires** the reachability plane into the data-plane effect: it proves that on `PunchError::NotReachable`, the relay endpoint from `drive_with_relay_fallback` flows into `apply_tunnel_plan`'s `peer_endpoint_override`, and that doing so **does not** disturb `wireguard-upgrade`'s validator `relay_candidates`.

- [ ] **Step 1: Add the dev-dependency**

In `crates/system/wireguard-effect/Cargo.toml`, under `[dev-dependencies]` (leave the existing `commonware-cryptography` line):

```toml
nat-traversal = { workspace = true, features = ["simnat"] }
```

- [ ] **Step 2: Write the failing integration test (RED)**

Create `crates/system/wireguard-effect/tests/relay_fallback.rs`. The `two_party_plan` fixture is copied verbatim from `src/wiring.rs`'s test module (a `TunnelInstallPlan` has no public constructor by design, so the fixture runs the real signed handshake):

```rust
use nat_traversal::{Coordinator, FallbackOutcome, NodeKey, SimNat, drive_with_relay_fallback};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use wireguard_effect::{FakeWireGuardEffect, apply_tunnel_plan};

use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
use wireguard_upgrade::*;

fn id(sk: &PrivateKey) -> ValidatorIdentity {
    ValidatorIdentity::try_from(sk.public_key().as_ref()).unwrap()
}

fn xkey(byte: u8) -> X25519PublicKey {
    X25519PublicKey([byte; 32])
}

fn endpoint(policy: &PortPolicy, addr: [u8; 4], port: u16, transport: Transport) -> Endpoint {
    Endpoint::new(IpAddr::V4(Ipv4Addr::from(addr)), port, transport, policy).unwrap()
}

/// A minimal two-validator handshake, direct (no validator relay), yielding the
/// initiator's validated install plan and its listen endpoint. Copied from
/// `wireguard-effect`'s `src/wiring.rs` test fixture; `relay_candidates` is
/// empty, which the invariant test below relies on.
fn two_party_plan() -> (TunnelInstallPlan, Endpoint) {
    let a = PrivateKey::from_seed(1);
    let b = PrivateKey::from_seed(2);
    let policy = PortPolicy::production();
    let overlay = OverlayPolicy::default_v4();
    let set = ActiveValidatorSet::new(
        "ducktape-wiring",
        1,
        Root([1u8; 32]),
        AdmissionRoot([2u8; 32]),
        vec![id(&a), id(&b)],
    )
    .unwrap();

    let record_a = EndpointRecord {
        namespace: set.namespace.clone(),
        epoch: set.epoch,
        valset_root: set.valset_root,
        admission_root: set.admission_root,
        validator_identity: id(&a),
        control_endpoint: endpoint(&policy, [1, 1, 1, 10], 443, Transport::Tcp),
        wireguard_endpoint: endpoint(&policy, [8, 8, 8, 10], 51820, Transport::Udp),
        capabilities: vec![],
        expires_at_view: 50,
        nonce: 1,
    };
    let record_b = EndpointRecord {
        namespace: set.namespace.clone(),
        epoch: set.epoch,
        valset_root: set.valset_root,
        admission_root: set.admission_root,
        validator_identity: id(&b),
        control_endpoint: endpoint(&policy, [1, 1, 1, 20], 443, Transport::Tcp),
        wireguard_endpoint: endpoint(&policy, [8, 8, 8, 20], 51820, Transport::Udp),
        capabilities: vec![],
        expires_at_view: 50,
        nonce: 1,
    };
    let records = vec![record_a.clone(), record_b.clone()];
    let mesh_version = compute_mesh_version(&records).unwrap();
    let ads = vec![
        EndpointAdvertisement::sign(record_a.clone(), mesh_version, &a),
        EndpointAdvertisement::sign(record_b.clone(), mesh_version, &b),
    ];
    let view = MeshView::verify(set.clone(), ads, &policy, 10).unwrap();

    let request = TunnelUpgradeRequest::sign(
        TunnelUpgradeRequestFields {
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            initiator_wireguard_public_key: xkey(0x0a),
            initiator_wireguard_endpoint: record_a.wireguard_endpoint,
            requested_allowed_ips: overlay.allowed_ips_for(&view, id(&b)).unwrap(),
            port_policy_hash: policy.hash(),
            expires_at_view: 40,
            nonce: 1,
        },
        &a,
    );
    let response = TunnelUpgradeResponse::sign(
        TunnelUpgradeResponseFields {
            request_hash: request.hash(),
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            responder_identity: id(&b),
            initiator_identity: id(&a),
            responder_wireguard_public_key: xkey(0x0b),
            responder_wireguard_endpoint: record_b.wireguard_endpoint,
            accepted_allowed_ips: overlay.allowed_ips_for(&view, id(&a)).unwrap(),
            relay_candidates: vec![],
            direct_dial_failure: None,
            keepalive_seconds: Some(25),
            expires_at_view: 40,
            nonce: 1,
        },
        &b,
    );
    let ack = TunnelUpgradeAck::sign(
        TunnelUpgradeAckFields {
            request_hash: request.hash(),
            response_hash: response.hash(),
            namespace: set.namespace.clone(),
            epoch: set.epoch,
            valset_root: set.valset_root,
            admission_root: set.admission_root,
            mesh_version: view.mesh_version,
            initiator_identity: id(&a),
            responder_identity: id(&b),
            installed_at_view: 11,
            expires_at_view: 40,
            nonce: 2,
        },
        &a,
    );

    let mut replay = ReplayCache::default();
    let plan = validate_upgrade(
        &view, &policy, &overlay, 12, &request, &response, &ack, &mut replay,
    )
    .unwrap();
    (plan, record_a.wireguard_endpoint)
}

#[test]
fn hole_punch_failure_relays_via_peer_endpoint_override() {
    // 1. A symmetric-NAT pair cannot hole-punch; the reachability plane falls
    //    back to the coordinator relay and returns the relay endpoint each side
    //    points WireGuard at.
    let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
    let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
    let mut coord = Coordinator::new();
    let outcome = drive_with_relay_fallback(
        NodeKey([0xaa; 32]),
        NodeKey([0xbb; 32]),
        &mut a_nat,
        &mut b_nat,
        &mut coord,
        b"a",
        b"b",
    )
    .expect("relay fallback");
    let relay_endpoint = match outcome {
        FallbackOutcome::Relayed(p) => p.a_relay_endpoint,
        FallbackOutcome::Punched { .. } => panic!("symmetric pair must not punch"),
    };

    // 2. The relay endpoint is wired into WireGuard EXACTLY through the Slice 0b
    //    seam: apply_tunnel_plan's peer_endpoint_override. No wireguard-upgrade
    //    plan surgery.
    let (plan, listen) = two_party_plan();
    let mut fake = FakeWireGuardEffect::default();
    apply_tunnel_plan(
        &mut fake,
        "ducktape-wg0",
        "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
        listen,
        &plan,
        Some(relay_endpoint),
    )
    .unwrap();

    assert_eq!(fake.applied[0].peers[0].endpoint, Some(relay_endpoint));
}

#[test]
fn coordinator_relay_never_touches_the_validator_relay_mechanism() {
    // INVARIANT: the reachability-plane coordinator relay is a DIFFERENT layer
    // from wireguard-upgrade's validator-only relay_candidates /
    // DirectDialFailureEvidence. Relaying through peer_endpoint_override must
    // leave the validated plan's relay_candidates untouched (empty here) — the
    // data plane's "relay must be a validator" rule is preserved, and the two
    // relay concepts never couple.
    let (plan, listen) = two_party_plan();
    assert!(
        plan.relay_candidates().is_empty(),
        "the fixture has no validator relay to begin with"
    );

    let relay_endpoint: SocketAddr = "192.0.2.1:4000".parse().unwrap();
    let mut fake = FakeWireGuardEffect::default();
    apply_tunnel_plan(
        &mut fake,
        "ducktape-wg0",
        "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
        listen,
        &plan,
        Some(relay_endpoint),
    )
    .unwrap();

    // The applied config carries the coordinator relay endpoint but the plan's
    // validator relay set is STILL empty: reachability-plane relay and
    // data-plane validator relay stayed separate.
    assert_eq!(fake.applied[0].peers[0].endpoint, Some(relay_endpoint));
    assert!(plan.relay_candidates().is_empty());
}
```

- [ ] **Step 3: Run to verify it fails, then passes**

Run: `cargo test -p wireguard-effect --test relay_fallback`
Expected: initially FAIL if the dev-dep/imports are missing; after Step 1 + the file compiles, PASS (2 tests).

- [ ] **Step 4: Full `wireguard-effect` gate**

Run: `cargo test -p wireguard-effect && cargo clippy -p wireguard-effect --all-targets -- -D warnings`
Expected: all green (existing `wiring`/`lib` unit tests + the new integration tests).

- [ ] **Step 5: Commit**

```bash
git add crates/system/wireguard-effect/Cargo.toml crates/system/wireguard-effect/tests/relay_fallback.rs
git commit -m "test(wireguard-effect): relay fallback via peer_endpoint_override + two-relay separation invariant"
```

---

### Task 8: Scoped merge gate + deferred-work note

**Files:**
- (Only lint fixes, if any, surface here.)

**Interfaces:** none — this task runs the full scoped gate and records what Slice 2 deliberately defers.

- [ ] **Step 1: Run the scoped test gate**

```bash
cargo test -p nat-traversal --features simnat \
  && cargo test -p wireguard-effect \
  && cargo test -p coordinator-bin
```

Expected: all green. (`coordinator-bin` is included as a sanity check because it consumes `run_coordinator`; it is not `bin/node`/`noded`, whose clippy is pre-existingly red.)

- [ ] **Step 2: Run the scoped clippy gate**

```bash
cargo clippy -p nat-traversal --features simnat --all-targets -- -D warnings \
  && cargo clippy -p wireguard-effect --all-targets -- -D warnings \
  && cargo clippy -p coordinator-bin --all-targets -- -D warnings
```

Expected: no warnings. **Do not** add `bin/node` / `noded` to the gate — their clippy is red from toolchain drift in unrelated dep crates, unrelated to this slice.

- [ ] **Step 3: Fix any lint findings and commit (only if Step 2 flagged something)**

```bash
git add -A
git commit -m "chore(nat-traversal): clippy clean across the Slice 2 relay surface"
```

- [ ] **Step 4: Confirm the deferred list matches reality**

See "What this plan deliberately does NOT do" below; nothing there should have been silently implemented.

---

## What this plan deliberately does NOT do (deferred / out of scope)

- **No `DirectDialFailureEvidence` wiring.** Per the epic invariant (§6 "two relay concepts, cleanly separated"), the signed `DirectDialFailureEvidence` is the *validator* relay's trigger and stays in `wireguard-upgrade`, untouched. The coordinator relay's trigger is `PunchError::NotReachable`. Coupling them would hand the untrusted coordinator a consensus artifact. (This is the one place this plan diverges from the design doc's Slice-2 line; see "Reconciliation with the design doc" above.)
- **No inviter-signed `RelayRequest`/`Register` authentication.** The coordinator remains unauthenticated at the message layer in this slice, exactly as Slices 0a/1 left it; authentication rests on the invite's signed `expected_key`. Wiring inviter-signed tokens onto `RelayRequest` is Slice 3 hardening (`Coordinator::request_relay` is written so a signature check is a local change).
- **No NAT rebinding / re-advertisement, no multiple-coordinator failover, no keepalive-survival tests** — Slice 3 hardening, alongside completing the full CI simulated-NAT suite from the design doc's Acceptance §1.
- **No real WireGuard-over-relay cross-machine run.** This slice proves the splice deterministically (SimNat) and over loopback (`run_relay_pair`); driving real WireGuard ciphertext through the relay on two boxes behind real NAT is Slice 3/4 (`docs/superpowers/specs/...coordinator-design.md` Acceptance §2).
- **No control-table wall-clock prune wired into `run_coordinator`.** The relay is bounded two ways already: the pure `Coordinator::prune_relays` (logical ticks, unit-tested) and each `run_relay_pair`'s `tokio` idle timeout (30s in the real loop, tested). Driving `prune_relays` off a periodic timer in the async loop is a Slice 3 cleanup.
- **No `bin/coordinator` CLI surface change.** The relay is served automatically because `run_coordinator`'s signature is unchanged; a `--relay-idle`/`--no-relay` flag is deferred.

## Self-review notes

- **Spec coverage.** Implements design-doc component ⑥ (coordinator ciphertext relay + relay fallback) and Acceptance §1's "hole-punch failure → coordinator relay splice" bullet. The reflexive/hole-punch pieces (⑤③④) are Slices 0a/0b; v3 invite (②) is Slice 1.
- **Requirement mapping.** (1) Coordinator ciphertext relay = Tasks 1–3 (control msgs, session table, opaque splice) + Task 6 (real serving); it never decrypts, holds only `SocketAddr` pairs (+ public `NodeKey`s for pairing, as rendezvous already does), and is idle-bounded. (2) Relay fallback via `peer_endpoint_override = relay endpoint`, punch-first = Task 5 (`FallbackOutcome::Punched` vs `Relayed`) + Task 7 (the `apply_tunnel_plan` wiring). (3) The two-relay separation invariant = Task 7's `coordinator_relay_never_touches_the_validator_relay_mechanism` + the global constraint that `nat-traversal` gains no validator-identity dependency. (4) Deterministic CI = Task 4 (`SimNat::symmetric` → `NotReachable`) + Task 5 asserting **actual opaque byte delivery** both ways, not filter state.
- **Type consistency.** `Msg::{RelayRequest, RelayGrant}`, `Side`, `RelaySplice`/`Forward`, `Coordinator::{request_relay, prune_relays}`, `FallbackOutcome`/`RelayFallbackProof`/`drive_with_relay_fallback`, `run_relay_pair`, `NatClient::{request_relay, relay_send, relay_recv}` are named identically across every task and their re-exports in `lib.rs`.
- **No placeholders.** Every step carries real code and an exact `cargo` command with its expected result; the `wireguard-effect` integration test copies the real `two_party_plan` handshake fixture rather than a stub, because `TunnelInstallPlan` has no public constructor.
- **Gate scoping.** `nat-traversal` (+ `wireguard-effect`, wired via a dev-dep) + a `coordinator-bin` sanity build; `bin/node`/`noded` are excluded by design (pre-existing clippy red from unrelated toolchain drift).
