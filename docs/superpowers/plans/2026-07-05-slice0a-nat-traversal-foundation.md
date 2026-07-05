# Slice 0a — NAT-Traversal Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the deterministic NAT-traversal primitive — reflexive-address discovery plus UDP simultaneous-open hole-punch, mediated by a minimal untrusted coordinator — so two dial-out-only endpoints behind (simulated) NAT can reach each other with no inbound port on either side.

**Architecture:** A new leaf crate `crates/system/nat-traversal` holds a tiny UDP control protocol (`Msg`), a `Coordinator` message handler (reflexive echo + rendezvous map), a `NatClient` (discover reflexive addr, register, look up a peer, run simultaneous-open), and an in-process `SimNat` that models NAT so hole-punch is testable deterministically in CI with no real network. A thin `bin/coordinator` binary wraps `Coordinator` over a real UDP socket. No WireGuard, no defguard, no consensus touched — this is the reachability primitive both hole-punch-to-WireGuard (Slice 0b) and the ciphertext relay (Slice 2) sit on.

**Tech Stack:** Rust (edition 2024), `tokio` (UDP + rt), the workspace's existing crate conventions. Wire encoding is hand-rolled fixed-layout bytes (same reader/`take` style as `bin/node/src/config.rs`'s v2 pack/unpack), no serde on the hot path.

## Global Constraints

- Edition: `edition.workspace = true` (2024), `version.workspace = true` — copy from any sibling in `crates/system/`.
- The coordinator is **untrusted**: it never holds or sees any key material beyond opaque bytes, never decrypts, never authorizes mesh membership. It only maps `node_key → reflexive SocketAddr` and echoes reflexive addresses. Keep it that way — no field on any coordinator type may carry private keys or plaintext payloads.
- Node identity is `commonware_cryptography::ed25519::PublicKey` (32 bytes). Reuse it; do not invent a new key type. In this crate a node is addressed by its raw 32-byte key (`NodeKey([u8; 32])`) to avoid a hard dep on the full signer surface.
- All wire integers are big-endian, fixed width. Every decode is bounds-checked and returns `Result<_, WireError>`; a short or malformed buffer is an error, never a panic.
- No `unwrap()`/`expect()` in library code paths except in tests.

---

### Task 1: Scaffold the crate + wire message types

**Files:**
- Create: `crates/system/nat-traversal/Cargo.toml`
- Create: `crates/system/nat-traversal/src/lib.rs`
- Create: `crates/system/nat-traversal/src/wire.rs`
- Modify: `Cargo.toml` (workspace root) — add `"crates/system/nat-traversal"` to `members`
- Test: inline `#[cfg(test)]` in `src/wire.rs`

**Interfaces:**
- Produces:
  - `pub struct NodeKey(pub [u8; 32])` (derives `Clone, Copy, Debug, PartialEq, Eq, Hash`)
  - `pub enum Msg { BindRequest { from: NodeKey }, BindResponse { reflexive: SocketAddr }, Register { key: NodeKey }, Lookup { key: NodeKey }, LookupResponse { key: NodeKey, reflexive: Option<SocketAddr> }, PunchSync { peer: NodeKey, peer_reflexive: SocketAddr }, Punch { from: NodeKey } }`
  - `impl Msg { pub fn encode(&self) -> Vec<u8>; pub fn decode(buf: &[u8]) -> Result<Msg, WireError>; }`
  - `pub enum WireError { Short, BadTag(u8), BadAddr }` (derives `Debug, PartialEq`)

- [ ] **Step 1: Add the crate to the workspace members list**

In `Cargo.toml` (root), inside `members = [ ... ]`, add after the `wireguard-upgrade` line:

```toml
    "crates/system/nat-traversal",
```

- [ ] **Step 2: Write `Cargo.toml` for the crate**

```toml
[package]
name = "nat-traversal"
edition.workspace = true
version.workspace = true

[dependencies]
commonware-cryptography.workspace = true
thiserror.workspace = true
tokio = { workspace = true, features = ["net", "rt", "time", "sync", "macros"] }

[dev-dependencies]
tokio = { workspace = true, features = ["net", "rt", "time", "sync", "macros", "rt-multi-thread"] }
```

If `tokio` is not yet a `[workspace.dependencies]` entry, add `tokio = { version = "1", default-features = false }` to the root `[workspace.dependencies]` and let features be selected per-crate as above.

- [ ] **Step 3: Write the failing roundtrip test**

In `crates/system/nat-traversal/src/wire.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(10, 0, 0, o)), p)
    }

    #[test]
    fn every_variant_roundtrips() {
        let cases = vec![
            Msg::BindRequest { from: NodeKey([1u8; 32]) },
            Msg::BindResponse { reflexive: addr(2, 51820) },
            Msg::Register { key: NodeKey([3u8; 32]) },
            Msg::Lookup { key: NodeKey([4u8; 32]) },
            Msg::LookupResponse { key: NodeKey([5u8; 32]), reflexive: Some(addr(6, 443)) },
            Msg::LookupResponse { key: NodeKey([7u8; 32]), reflexive: None },
            Msg::PunchSync { peer: NodeKey([8u8; 32]), peer_reflexive: addr(9, 7000) },
            Msg::Punch { from: NodeKey([10u8; 32]) },
        ];
        for m in cases {
            let bytes = m.encode();
            let back = Msg::decode(&bytes).expect("decode");
            assert_eq!(m, back);
        }
    }

    #[test]
    fn short_buffer_is_error() {
        assert_eq!(Msg::decode(&[]), Err(WireError::Short));
        assert_eq!(Msg::decode(&[0xff]), Err(WireError::BadTag(0xff)));
    }
}
```

- [ ] **Step 4: Run it to confirm it fails to compile / fails**

Run: `cargo test -p nat-traversal wire::`
Expected: FAIL — `Msg`, `NodeKey`, `WireError` not defined.

- [ ] **Step 5: Implement `wire.rs`**

```rust
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeKey(pub [u8; 32]);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Msg {
    BindRequest { from: NodeKey },
    BindResponse { reflexive: SocketAddr },
    Register { key: NodeKey },
    Lookup { key: NodeKey },
    LookupResponse { key: NodeKey, reflexive: Option<SocketAddr> },
    PunchSync { peer: NodeKey, peer_reflexive: SocketAddr },
    Punch { from: NodeKey },
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum WireError {
    #[error("buffer too short")]
    Short,
    #[error("bad tag {0}")]
    BadTag(u8),
    #[error("bad address encoding")]
    BadAddr,
}

const TAG_BIND_REQ: u8 = 1;
const TAG_BIND_RESP: u8 = 2;
const TAG_REGISTER: u8 = 3;
const TAG_LOOKUP: u8 = 4;
const TAG_LOOKUP_RESP: u8 = 5;
const TAG_PUNCH_SYNC: u8 = 6;
const TAG_PUNCH: u8 = 7;

fn put_key(out: &mut Vec<u8>, k: &NodeKey) {
    out.extend_from_slice(&k.0);
}

fn put_addr(out: &mut Vec<u8>, a: &SocketAddr) {
    match a.ip() {
        IpAddr::V4(v4) => {
            out.push(4);
            out.extend_from_slice(&v4.octets());
        }
        IpAddr::V6(v6) => {
            out.push(6);
            out.extend_from_slice(&v6.octets());
        }
    }
    out.extend_from_slice(&a.port().to_be_bytes());
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], WireError> {
        let end = self.pos.checked_add(n).ok_or(WireError::Short)?;
        if end > self.buf.len() {
            return Err(WireError::Short);
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn key(&mut self) -> Result<NodeKey, WireError> {
        let s = self.take(32)?;
        let mut k = [0u8; 32];
        k.copy_from_slice(s);
        Ok(NodeKey(k))
    }
    fn addr(&mut self) -> Result<SocketAddr, WireError> {
        let fam = self.take(1)?[0];
        let ip = match fam {
            4 => {
                let o = self.take(4)?;
                IpAddr::V4(Ipv4Addr::new(o[0], o[1], o[2], o[3]))
            }
            6 => {
                let o = self.take(16)?;
                let mut b = [0u8; 16];
                b.copy_from_slice(o);
                IpAddr::V6(Ipv6Addr::from(b))
            }
            _ => return Err(WireError::BadAddr),
        };
        let p = self.take(2)?;
        let port = u16::from_be_bytes([p[0], p[1]]);
        Ok(SocketAddr::new(ip, port))
    }
}

impl Msg {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(48);
        match self {
            Msg::BindRequest { from } => {
                out.push(TAG_BIND_REQ);
                put_key(&mut out, from);
            }
            Msg::BindResponse { reflexive } => {
                out.push(TAG_BIND_RESP);
                put_addr(&mut out, reflexive);
            }
            Msg::Register { key } => {
                out.push(TAG_REGISTER);
                put_key(&mut out, key);
            }
            Msg::Lookup { key } => {
                out.push(TAG_LOOKUP);
                put_key(&mut out, key);
            }
            Msg::LookupResponse { key, reflexive } => {
                out.push(TAG_LOOKUP_RESP);
                put_key(&mut out, key);
                match reflexive {
                    Some(a) => {
                        out.push(1);
                        put_addr(&mut out, a);
                    }
                    None => out.push(0),
                }
            }
            Msg::PunchSync { peer, peer_reflexive } => {
                out.push(TAG_PUNCH_SYNC);
                put_key(&mut out, peer);
                put_addr(&mut out, peer_reflexive);
            }
            Msg::Punch { from } => {
                out.push(TAG_PUNCH);
                put_key(&mut out, from);
            }
        }
        out
    }

    pub fn decode(buf: &[u8]) -> Result<Msg, WireError> {
        let mut r = Reader::new(buf);
        let tag = r.take(1)?[0];
        match tag {
            TAG_BIND_REQ => Ok(Msg::BindRequest { from: r.key()? }),
            TAG_BIND_RESP => Ok(Msg::BindResponse { reflexive: r.addr()? }),
            TAG_REGISTER => Ok(Msg::Register { key: r.key()? }),
            TAG_LOOKUP => Ok(Msg::Lookup { key: r.key()? }),
            TAG_LOOKUP_RESP => {
                let key = r.key()?;
                let present = r.take(1)?[0];
                let reflexive = match present {
                    0 => None,
                    1 => Some(r.addr()?),
                    _ => return Err(WireError::BadAddr),
                };
                Ok(Msg::LookupResponse { key, reflexive })
            }
            TAG_PUNCH_SYNC => Ok(Msg::PunchSync {
                peer: r.key()?,
                peer_reflexive: r.addr()?,
            }),
            TAG_PUNCH => Ok(Msg::Punch { from: r.key()? }),
            other => Err(WireError::BadTag(other)),
        }
    }
}
```

- [ ] **Step 6: Add module wiring to `lib.rs`**

```rust
//! nat-traversal: reflexive-address discovery + UDP hole-punch mediated by an
//! untrusted coordinator. No WireGuard, no consensus — the reachability
//! primitive under the private-cutover epic.

pub mod wire;

pub use wire::{Msg, NodeKey, WireError};
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cargo test -p nat-traversal wire::`
Expected: PASS (2 tests).

- [ ] **Step 8: Commit**

```bash
git add crates/system/nat-traversal/Cargo.toml crates/system/nat-traversal/src/lib.rs crates/system/nat-traversal/src/wire.rs Cargo.toml
git commit -m "feat(nat-traversal): crate scaffold + UDP control wire format"
```

---

### Task 2: Coordinator message handler (reflexive echo + rendezvous map)

**Files:**
- Create: `crates/system/nat-traversal/src/coordinator.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (add `pub mod coordinator;`)
- Test: inline `#[cfg(test)]` in `src/coordinator.rs`

**Interfaces:**
- Consumes: `Msg`, `NodeKey` from Task 1.
- Produces:
  - `pub struct Coordinator { /* private: HashMap<NodeKey, SocketAddr> */ }`
  - `impl Coordinator { pub fn new() -> Self; pub fn handle(&mut self, from: SocketAddr, msg: Msg) -> Vec<(SocketAddr, Msg)>; }`
  - `handle` is pure: it takes the sender's observed source address + the decoded message and returns zero or more `(dest, reply)` datagrams to send. This keeps the coordinator logic transport-free and unit-testable with no sockets.

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal coordinator::`
Expected: FAIL — `Coordinator` not defined.

- [ ] **Step 3: Implement `coordinator.rs`**

```rust
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
                        .find(|(_, &v)| v == from)
                        .map(|(k, _)| *k)
                        .unwrap_or(NodeKey([0u8; 32]));
                    out.push((from, Msg::PunchSync { peer: key, peer_reflexive: peer_addr }));
                    out.push((peer_addr, Msg::PunchSync { peer: caller_key, peer_reflexive: from }));
                }
                out
            }
            // The coordinator never receives BindResponse/LookupResponse/PunchSync/Punch;
            // those are node-directed. Ignore defensively.
            Msg::BindResponse { .. }
            | Msg::LookupResponse { .. }
            | Msg::PunchSync { .. }
            | Msg::Punch { .. } => Vec::new(),
        }
    }
}
```

- [ ] **Step 4: Add `pub mod coordinator;` to `lib.rs`**

```rust
pub mod coordinator;
pub use coordinator::Coordinator;
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal coordinator::`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/coordinator.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): transport-free coordinator handler (echo + rendezvous)"
```

---

### Task 3: `SimNat` — deterministic in-process NAT model

**Files:**
- Create: `crates/system/nat-traversal/src/simnat.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (`#[cfg(any(test, feature = "simnat"))] pub mod simnat;` — see step)
- Modify: `crates/system/nat-traversal/Cargo.toml` (add a `simnat` feature)
- Test: inline `#[cfg(test)]` in `src/simnat.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks (pure model).
- Produces:
  - `pub struct SimNat` modelling one endpoint-independent-mapping, address-dependent-filtering NAT (the realistic "restricted-cone" case hole-punch is designed for).
  - `impl SimNat { pub fn new(public_ip: IpAddr) -> Self; pub fn send(&mut self, internal_src: SocketAddr, dst: SocketAddr) -> SocketAddr; pub fn allow_inbound(&self, mapped: SocketAddr, from: SocketAddr) -> bool; }`
  - `send` returns the public `mapped` address for an outbound datagram and records that `internal_src` opened a hole toward `dst`. `allow_inbound` returns true only if a prior outbound to `from` created the mapping (filtering), which is exactly why simultaneous-open is required.

- [ ] **Step 1: Add the feature to `Cargo.toml`**

```toml
[features]
simnat = []
```

- [ ] **Step 2: Write the failing test**

```rust
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
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test -p nat-traversal --features simnat simnat::`
Expected: FAIL — `SimNat` not defined.

- [ ] **Step 4: Implement `simnat.rs`**

```rust
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
```

- [ ] **Step 5: Add module to `lib.rs`**

```rust
#[cfg(any(test, feature = "simnat"))]
pub mod simnat;
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat simnat::`
Expected: PASS (2 tests).

- [ ] **Step 7: Commit**

```bash
git add crates/system/nat-traversal/src/simnat.rs crates/system/nat-traversal/src/lib.rs crates/system/nat-traversal/Cargo.toml
git commit -m "feat(nat-traversal): deterministic restricted-cone SimNat model"
```

---

### Task 4: Hole-punch state machine over `SimNat` (the risk-killer test)

**Files:**
- Create: `crates/system/nat-traversal/src/punch.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (`pub mod punch;`)
- Test: inline `#[cfg(test)]` in `src/punch.rs`, driving two `SimNat`s + a `Coordinator` with NO real sockets.

**Interfaces:**
- Consumes: `Coordinator` (Task 2), `SimNat` (Task 3), `Msg`/`NodeKey` (Task 1).
- Produces:
  - `pub struct PunchPlan { pub local_mapped: SocketAddr, pub peer_reflexive: SocketAddr }`
  - `pub fn drive_simulated(a_key, b_key, a_nat, b_nat, coord) -> Result<(PunchPlan, PunchPlan), PunchError>` — a deterministic in-memory choreography: both nodes Register (learning their mapped addr via the coordinator's echo path), A issues Lookup, the coordinator emits PunchSync to both, both send `Punch` to the other's reflexive, and the function asserts each side's NAT now permits the other's inbound. This is the executable proof that two dial-out-only endpoints reach each other with no inbound port. `PunchError` covers the failure (e.g. one side never opened its hole → symmetric-NAT-like fallback needed, which Slice 2 relay handles).

- [ ] **Step 1: Write the failing test**

```rust
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
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: FAIL — `drive_simulated` not defined.

- [ ] **Step 3: Implement `punch.rs`**

```rust
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
```

- [ ] **Step 4: Add module to `lib.rs`**

```rust
pub mod punch;
pub use punch::{PunchError, PunchPlan};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: PASS (1 test) — the risk-killer green.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/punch.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): simultaneous-open hole-punch proven over SimNat"
```

---

### Task 5: `NatClient` over real tokio UDP sockets

**Files:**
- Create: `crates/system/nat-traversal/src/client.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (`pub mod client;`)
- Test: inline `#[cfg(test)]` async test in `src/client.rs` running a real `Coordinator` task on loopback.

**Interfaces:**
- Consumes: `Msg`, `NodeKey`, `Coordinator`.
- Produces:
  - `pub struct NatClient { /* UdpSocket, NodeKey */ }`
  - `impl NatClient { pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self>; pub async fn discover_reflexive(&self) -> std::io::Result<SocketAddr>; pub async fn register(&self) -> std::io::Result<()>; pub async fn local_addr(&self) -> std::io::Result<SocketAddr>; }`
  - A `pub async fn run_coordinator(sock: UdpSocket)` helper that loops decoding datagrams, feeding a `Coordinator`, and sending replies — the guts of `bin/coordinator`.

- [ ] **Step 1: Write the failing async test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::NodeKey;
    use tokio::net::UdpSocket;

    #[tokio::test]
    async fn client_discovers_its_reflexive_via_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        tokio::spawn(run_coordinator(coord_sock));

        let client = NatClient::bind(NodeKey([1u8; 32]), coord_addr).await.unwrap();
        let reflexive = client.discover_reflexive().await.unwrap();
        // The socket binds 0.0.0.0:0, so local_addr() reports the wildcard IP
        // while the coordinator observes 127.0.0.1 as the source — the IPs
        // differ by design. The port is the load-bearing invariant.
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal client::`
Expected: FAIL — `NatClient`, `run_coordinator` not defined.

- [ ] **Step 3: Implement `client.rs`**

```rust
use std::net::SocketAddr;

use tokio::net::UdpSocket;

use crate::{Coordinator, Msg, NodeKey};

pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
}

impl NatClient {
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord })
    }

    pub async fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.sock.local_addr()
    }

    pub async fn discover_reflexive(&self) -> std::io::Result<SocketAddr> {
        self.sock
            .send_to(&Msg::BindRequest { from: self.key }.encode(), self.coord)
            .await?;
        let mut buf = [0u8; 64];
        loop {
            let (n, _from) = self.sock.recv_from(&mut buf).await?;
            if let Ok(Msg::BindResponse { reflexive }) = Msg::decode(&buf[..n]) {
                return Ok(reflexive);
            }
        }
    }

    pub async fn register(&self) -> std::io::Result<()> {
        self.sock
            .send_to(&Msg::Register { key: self.key }.encode(), self.coord)
            .await?;
        Ok(())
    }
}

/// The coordinator event loop: decode, feed the pure handler, send replies.
pub async fn run_coordinator(sock: UdpSocket) {
    let mut coord = Coordinator::new();
    let mut buf = [0u8; 64];
    loop {
        let (n, from) = match sock.recv_from(&mut buf).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let msg = match Msg::decode(&buf[..n]) {
            Ok(m) => m,
            Err(_) => continue,
        };
        for (dst, reply) in coord.handle(from, msg) {
            let _ = sock.send_to(&reply.encode(), dst).await;
        }
    }
}
```

- [ ] **Step 4: Add module to `lib.rs`**

```rust
pub mod client;
pub use client::{NatClient, run_coordinator};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal client::`
Expected: PASS (1 test).

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/client.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): NatClient + coordinator event loop over tokio UDP"
```

---

### Task 6: `bin/coordinator` runnable binary

**Files:**
- Create: `bin/coordinator/Cargo.toml`
- Create: `bin/coordinator/src/main.rs`
- Modify: `Cargo.toml` (workspace root) — add `"bin/coordinator"` to `members`
- Test: a smoke test — `tests/smoke.rs` that boots the binary's `run_coordinator` on an ephemeral port and round-trips one `BindRequest`.

**Interfaces:**
- Consumes: `nat_traversal::{run_coordinator}` and the `NatClient` for the smoke test.
- Produces: an executable `coordinator` that binds `--listen <addr>` (default `0.0.0.0:3478`) and serves forever. This is the seed of `p2p.ducktape.industries`.

- [ ] **Step 1: Write `bin/coordinator/Cargo.toml`**

```toml
[package]
name = "coordinator-bin"
edition.workspace = true
version.workspace = true

[[bin]]
name = "coordinator"
path = "src/main.rs"

[dependencies]
nat-traversal = { workspace = true }
tokio = { workspace = true, features = ["net", "rt-multi-thread", "macros"] }

[dev-dependencies]
nat-traversal = { workspace = true, features = ["simnat"] }
tokio = { workspace = true, features = ["net", "rt-multi-thread", "macros"] }
```

Add `nat-traversal = { path = "crates/system/nat-traversal" }` to root `[workspace.dependencies]` so `workspace = true` resolves.

- [ ] **Step 2: Add to workspace members**

In root `Cargo.toml` `members`, under the `# bin — runnable binaries` group, add:

```toml
    "bin/coordinator",
```

- [ ] **Step 3: Write the failing smoke test**

`bin/coordinator/tests/smoke.rs`:

```rust
use nat_traversal::{NatClient, NodeKey, run_coordinator};
use tokio::net::UdpSocket;

#[tokio::test]
async fn coordinator_answers_a_bind_request() {
    let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let addr = sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(sock));

    let client = NatClient::bind(NodeKey([9u8; 32]), addr).await.unwrap();
    let reflexive = client.discover_reflexive().await.unwrap();
    // Wildcard bind vs observed loopback source: compare the port, not the IP.
    assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
}
```

- [ ] **Step 4: Run to verify it fails**

Run: `cargo test -p coordinator-bin`
Expected: FAIL — binary/target not present or test cannot resolve until `main.rs` exists.

- [ ] **Step 5: Write `bin/coordinator/src/main.rs`**

```rust
use std::net::SocketAddr;

use nat_traversal::run_coordinator;
use tokio::net::UdpSocket;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let listen: SocketAddr = std::env::args()
        .skip_while(|a| a != "--listen")
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| "0.0.0.0:3478".parse().unwrap());

    let sock = UdpSocket::bind(listen).await?;
    eprintln!("coordinator listening on {}", sock.local_addr()?);
    run_coordinator(sock).await;
    Ok(())
}
```

- [ ] **Step 6: Run to verify pass + binary builds**

Run: `cargo test -p coordinator-bin && cargo build -p coordinator-bin`
Expected: PASS (1 test); binary builds.

- [ ] **Step 7: Commit**

```bash
git add bin/coordinator Cargo.toml
git commit -m "feat(coordinator): runnable bin/coordinator wrapping the event loop"
```

---

### Task 7: Two-client end-to-end punch over real sockets

**Files:**
- Create: `crates/system/nat-traversal/tests/punch_e2e.rs`

**Interfaces:**
- Consumes: `NatClient`, `run_coordinator`, `NodeKey`.
- Produces: an integration test proving two real UDP sockets, each registered with a real coordinator task, exchange datagrams directly after rendezvous (loopback stands in for "both reachable after punch"; the SimNat test in Task 4 covers the filtered-NAT proof). This closes Slice 0a: discovery + rendezvous + direct peer datagram, wired over real I/O.

- [ ] **Step 1: Write the failing test**

```rust
use nat_traversal::{Msg, NatClient, NodeKey, run_coordinator};
use tokio::net::UdpSocket;
use tokio::time::{Duration, timeout};

#[tokio::test]
async fn two_clients_rendezvous_and_send_directly() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock));

    let a = NatClient::bind(NodeKey([0xaa; 32]), coord_addr).await.unwrap();
    let b = NatClient::bind(NodeKey([0xbb; 32]), coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();

    // A resolves B via the coordinator and sends a direct datagram to B's
    // reflexive (== B's loopback addr here). B receives it.
    let b_addr = b.local_addr().await.unwrap();
    a.send_punch_to(b_addr).await.unwrap();

    let got = timeout(Duration::from_secs(2), b.recv_punch())
        .await
        .expect("no timeout")
        .expect("recv");
    assert_eq!(got, Msg::Punch { from: NodeKey([0xaa; 32]) });
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p nat-traversal --test punch_e2e`
Expected: FAIL — `send_punch_to` / `recv_punch` not defined.

- [ ] **Step 3: Add the two helpers to `NatClient` in `client.rs`**

```rust
    pub async fn send_punch_to(&self, peer: SocketAddr) -> std::io::Result<()> {
        self.sock
            .send_to(&Msg::Punch { from: self.key }.encode(), peer)
            .await?;
        Ok(())
    }

    pub async fn recv_punch(&self) -> std::io::Result<Msg> {
        let mut buf = [0u8; 64];
        loop {
            let (n, _from) = self.sock.recv_from(&mut buf).await?;
            if let Ok(m @ Msg::Punch { .. }) = Msg::decode(&buf[..n]) {
                return Ok(m);
            }
        }
    }
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nat-traversal --test punch_e2e`
Expected: PASS (1 test).

- [ ] **Step 5: Full crate gate + clippy**

Run: `cargo test -p nat-traversal --all-features && cargo clippy -p nat-traversal --all-features -- -D warnings`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/tests/punch_e2e.rs crates/system/nat-traversal/src/client.rs
git commit -m "test(nat-traversal): two-client rendezvous + direct datagram e2e"
```

---

## What this plan deliberately does NOT do (deferred to later plans)

- **Slice 0b — WireGuard effect adapter.** Wire the punched path into a real WireGuard interface via `defguard_wireguard_rs` 0.10 behind a `WireGuardEffect` trait (fake for CI, defguard for real). This is where the `wireguard-upgrade` `TunnelInstallPlan → DefguardInterfaceConfig → WGApi::configure_interface` seam finally gets a consumer, and where the pinned responder-perspective plan gap (`validate_upgrade` emits initiator-local only) must be resolved. Kept separate because it needs the defguard API verified at that plan's writing time.
- **Cross-machine acceptance** (real NAT, two boxes, coordinator on a third) — a runbook task in a later slice.
- **v3 signed invite / typed `Reach`** — Slice 1.
- **Ciphertext relay + relay fallback** — Slice 2. `PunchError::NotReachable` is the trigger seam.
- Authentication of `Register`/`Lookup` by inviter-signed tokens — folded into Slice 1 when the invite format lands. Task 2's handler is written so adding a signature check is a local change.

## Self-review notes

- **Spec coverage:** implements the STUN reflexive (Task 2/5), hole-punch simultaneous-open (Task 4), and minimal `bin/coordinator` (Task 6) from spec §2 components ①③④ and the Slice 0 decomposition. Relay (⑥), v3 invite (②), and WGApi wiring (⑤) are explicitly deferred above with their spec anchors.
- **Type consistency:** `Msg`/`NodeKey`/`Coordinator`/`SimNat`/`NatClient`/`run_coordinator` names are used identically across tasks; `handle` returns `Vec<(SocketAddr, Msg)>` everywhere; `drive_simulated` and `PunchPlan` fields match their test usage.
- **No placeholders:** every step carries real code and an exact `cargo` command with expected result.
