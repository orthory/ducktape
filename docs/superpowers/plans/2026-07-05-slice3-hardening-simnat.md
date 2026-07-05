# Slice 3 — Hardening (Rebinding · Multi-Coordinator · Survival) + Full CI Simulated-NAT Suite

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task ends in a commit; do not batch commits.

**Goal:** Harden the reachability plane built in Slices 0a/0b/1/2 against the three failure modes the design's §"Error and fallback handling" enumerates — **NAT rebinding**, **coordinator unreachability (multiple coordinators)**, and **path survival across coordinator downtime** — and consolidate the deterministic proofs into the single **CI simulated-NAT suite** that is the epic's merge gate (design §"Acceptance" item 1). This slice is the last of the code slices before the Slice 4 deployment runbook.

**Scope boundary (what this slice extends).** Everything lands in `crates/system/nat-traversal`, extending the already-built primitives:

- `wire.rs` — `Msg` codec (`Register`/`Lookup`/`PunchSync`/`Punch`/`RelayRequest`/`RelayGrant`, `NodeKey`, big-endian bounds-checked reader, trailing-byte rejection).
- `coordinator.rs` — `Coordinator` (reflexive registry, rendezvous fan-out, `request_relay`/`release_relay`/`prune_relays`, `Side`).
- `simnat.rs` — `SimNat::new` (restricted-cone) + `SimNat::symmetric` (per-destination port), `send`/`allow_inbound`.
- `punch.rs` (`--features simnat`) — `drive_simulated` (returns `PunchError::NotReachable` for symmetric), `drive_with_relay_fallback` (`FallbackOutcome::{Punched,Relayed}`, `RelayFallbackProof` asserting real byte delivery), `punch_once`.
- `relay.rs` — `RelaySplice` (learn-on-first, source-pinned, idle-bounded) + `Forward`.
- `client.rs` — `NatClient`, `run_coordinator`/`run_coordinator_with_idle`, `run_relay_pair`.

**Design anchors.** `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`:

- §"Error and fallback handling": "Coordinator unreachable → try the other hints in the `Vec` … already-established connections survive via keepalive. The coordinator is not uniquely load-bearing." / "NAT rebinding → re-run STUN and re-advertise under a higher monotonic nonce, respecting the duplicate-advertisement rule."
- §"Acceptance" item 1 (CI simulated-NAT suite, the merge gate): "deterministically covers: reflexive discovery; hole-punch success; hole-punch failure → coordinator relay splice; v3 invite signature verify and reject; v2 parse-compatibility; endpoint-churn re-advertisement."
- §"Epic decomposition": "Slice 3. Hardening: NAT rebinding, multiple coordinators, keepalive survival; complete the CI simulated-NAT suite."
- The monotonic-nonce rule this slice mirrors is `wireguard_upgrade::MeshView::verify` in `crates/system/wireguard-upgrade/src/lib.rs` (~line 344): a duplicate advertisement with `nonce <= prev.record.nonce` is `StaleDuplicateAdvertisement`; a strictly-higher nonce supersedes.

**The load-bearing invariant this slice must not break (from Slice 2).** There are two different relays and they stay structurally separate. `nat-traversal` gains **no** dependency on `wireguard-upgrade` / validator-identity types. The monotonic-nonce re-advertisement rule is therefore **modeled locally** as a pure reachability-plane primitive (`advert.rs`) that *mirrors* `MeshView::verify`'s rule in a documented comment but never imports it. The coordinator relay remains a below-WireGuard opaque `SocketAddr` splice.

**Tech stack:** Rust (edition 2024), `tokio` (UDP + `time` + `select!`), workspace crate conventions. Hand-rolled fixed-layout wire bytes (no serde on the hot path). Pure models are transport-free and deterministic; the async layer provides the real runtime for survival tests.

## Global Constraints

- **Gate is `nat-traversal` only.** Every `cargo` command in this plan targets `-p nat-traversal`. Do **not** add `nat-traversal` → `wireguard-upgrade` / `bin/node` / `noded` edges, and do **not** run the workspace clippy: `bin/node`/`noded` clippy is pre-existingly red from toolchain drift in unrelated dep crates. The merge gate is exactly:
  ```bash
  cargo test -p nat-traversal --features simnat && \
  cargo clippy -p nat-traversal --features simnat --all-targets -- -D warnings
  ```
- **Don't disturb Slices 0–2.** The 31 existing unit tests + `tests/punch_e2e.rs` are green today. Every task re-runs the relevant subset; the final task re-runs the whole gate. Refactors (the `AdvertBook` adoption in Task 3, the `punch_until_bidirectional` extraction in Task 4) must be behavior-preserving — the existing tests are the safety net and must stay green with **no edits to their assertions**.
- **No `unwrap()`/`expect()` in library paths** except in `#[cfg(test)]`. All new wire/logic returns `Result`/`Option`; every decode stays bounds-checked; trailing bytes stay rejected.
- **Untrusted coordinator preserved.** New state holds only public `NodeKey`s, `SocketAddr`s, and `u64` nonces/session-ids — never key material, never plaintext, never a `ValidatorIdentity`.
- **Do not overclaim survival (Task 6).** A PUNCHED direct path is coordinator-independent after setup; a RELAYED path's data forwarding lives *inside the coordinator process* and its (re)establishment strictly requires a live coordinator. The tests encode exactly this asymmetry and nothing stronger. In particular, do **not** add a blind wall-clock prune that could tear down a live splice — Slice 2 deliberately reclaims relay state on splice-task completion (`run_coordinator_with_idle`'s `done_tx`/`done_rx` + `release_relay`), because the coordinator cannot observe data-plane relay activity. That reclaim IS the idle prune; this slice adds tests around it, not a competing mechanism.

## Scoping note on Acceptance §1 (read before Task 8)

Acceptance §1's list spans two crates. This slice's gate covers the `nat-traversal` portion:

| Acceptance §1 item | Where it lives | This slice |
|---|---|---|
| reflexive discovery | `nat-traversal` (STUN reflexive) | ✅ suite (Task 8) |
| hole-punch success | `nat-traversal` `drive_simulated` | ✅ strengthened (Task 7) + suite |
| hole-punch failure → relay splice | `nat-traversal` `drive_with_relay_fallback` | ✅ suite |
| endpoint-churn re-advertisement | `nat-traversal` (this slice, Tasks 1–4) | ✅ new + suite |
| v3 invite signature verify/reject | `bin/node/src/config.rs` (Slice 1) | ⛔ out of this gate — lives behind node-bin's pre-red clippy; verified in Slice 1's own tests |
| v2 parse-compatibility | `bin/node/src/config.rs` (Slice 1) | ⛔ out of this gate — same reason |

Task 8's suite doc comment records this table so the gate's coverage is unambiguous and the two node-bin items are visibly *referenced, delegated to Slice 1*, not silently dropped.

---

### Task 1: `advert.rs` — the monotonic-nonce reflexive-advertisement primitive

Pure, transport-free, always-compiled. This is the reachability-plane analog of `wireguard_upgrade::MeshView::verify`'s duplicate rule, implemented once here so the coordinator (Task 3) and the rebind driver (Task 4) share a single source of truth without depending on `wireguard-upgrade`.

**Files:**
- Create: `crates/system/nat-traversal/src/advert.rs`
- Modify: `crates/system/nat-traversal/src/lib.rs` (`pub mod advert;` + re-exports)
- Test: inline `#[cfg(test)]` in `src/advert.rs`

**Interfaces:**
- `pub struct ReflexiveAdvert { pub reflexive: SocketAddr, pub nonce: u64 }`
- `pub enum AdvertOutcome { Superseded, Stale }`
- `pub struct AdvertBook` (derives `Default`) — the per-key latest-advert registry:
  - `pub fn observe(&mut self, key: NodeKey, src: SocketAddr)` — boot/live registration: the coordinator-observed source is authoritative; establishes the baseline at nonce `0` (unconditional, matching the original `Register` overwrite).
  - `pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome` — rebind path: strictly-higher `nonce` supersedes (stores `src`, returns `Superseded`); `nonce <= prev` is `Stale` and leaves the stored advert **untouched**. Mirrors `MeshView::verify`'s `nonce <= prev.record.nonce => StaleDuplicateAdvertisement`.
  - `pub fn current(&self, key: NodeKey) -> Option<SocketAddr>`
  - `pub fn key_for_src(&self, src: SocketAddr) -> Option<NodeKey>` — reverse map (centralizes the `find(|(_,&v)| v == src)` the coordinator did twice).

- [ ] **Step 1: Write the failing tests (RED)**

`crates/system/nat-traversal/src/advert.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};

    fn addr(o: u8, p: u16) -> SocketAddr {
        SocketAddr::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, o)), p)
    }

    #[test]
    fn first_observe_then_higher_nonce_supersedes() {
        let key = NodeKey([0xaa; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000)); // boot: nonce 0
        assert_eq!(book.current(key), Some(addr(1, 4000)));

        // A rebind advertises the NEW reflexive under a strictly-higher nonce:
        // it supersedes the stale mapping.
        assert_eq!(book.readvertise(key, addr(2, 5000), 1), AdvertOutcome::Superseded);
        assert_eq!(book.current(key), Some(addr(2, 5000)));
        assert_eq!(book.key_for_src(addr(2, 5000)), Some(key));
        assert_eq!(book.key_for_src(addr(1, 4000)), None, "stale mapping is gone");
    }

    #[test]
    fn equal_or_lower_nonce_is_stale_and_does_not_change_mapping() {
        let key = NodeKey([0xbb; 32]);
        let mut book = AdvertBook::default();
        book.observe(key, addr(1, 4000)); // nonce 0
        assert_eq!(book.readvertise(key, addr(2, 5000), 2), AdvertOutcome::Superseded);

        // A replayed / equal-nonce advert must not clobber the fresher mapping
        // (mirrors StaleDuplicateAdvertisement: nonce <= prev).
        assert_eq!(book.readvertise(key, addr(9, 9999), 2), AdvertOutcome::Stale);
        assert_eq!(book.readvertise(key, addr(9, 9999), 1), AdvertOutcome::Stale);
        assert_eq!(book.current(key), Some(addr(2, 5000)), "stale adverts leave state untouched");
    }

    #[test]
    fn unknown_key_has_no_current() {
        let book = AdvertBook::default();
        assert_eq!(book.current(NodeKey([0xcc; 32])), None);
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal advert::`
Expected: FAIL — `advert` module / `AdvertBook` not defined.

- [ ] **Step 3: Implement `advert.rs`**

```rust
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::NodeKey;

/// One node's latest reflexive advertisement: the reflexive `SocketAddr` a node
/// published and the monotonic `nonce` that orders it. The nonce is an ordering
/// token only — the address is always the coordinator-observed source, never a
/// self-reported one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReflexiveAdvert {
    pub reflexive: SocketAddr,
    pub nonce: u64,
}

/// Result of applying a re-advertisement: it either superseded the stored
/// mapping or was rejected as stale.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdvertOutcome {
    Superseded,
    Stale,
}

/// The reachability-plane reflexive registry: for each node key, the latest
/// accepted `ReflexiveAdvert`. `observe` is the unconditional boot/live
/// registration (the observed source is authoritative); `readvertise` is the
/// nonce-gated rebind path.
///
/// The nonce rule deliberately MIRRORS `wireguard_upgrade::MeshView::verify`'s
/// duplicate-advertisement rule (`nonce <= prev => StaleDuplicateAdvertisement`)
/// so a NAT-rebound node re-advertises under a strictly-higher nonce to
/// supersede its stale mapping — WITHOUT this crate depending on
/// `wireguard-upgrade` or any validator-identity type (the Slice 2 invariant).
#[derive(Default)]
pub struct AdvertBook {
    latest: HashMap<NodeKey, ReflexiveAdvert>,
}

impl AdvertBook {
    /// Boot/live registration. The coordinator-observed `src` is authoritative;
    /// establish (or reset) the baseline at nonce 0. Matches the coordinator's
    /// original unconditional `Register` insert.
    pub fn observe(&mut self, key: NodeKey, src: SocketAddr) {
        self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce: 0 });
    }

    /// Rebind re-advertisement. A strictly-higher `nonce` supersedes the stored
    /// mapping (store `src`, return `Superseded`); an equal-or-lower nonce is
    /// stale and leaves the stored advert untouched (`Stale`). No prior entry ->
    /// accepted as a first advert.
    pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome {
        match self.latest.get(&key) {
            Some(prev) if nonce <= prev.nonce => AdvertOutcome::Stale,
            _ => {
                self.latest.insert(key, ReflexiveAdvert { reflexive: src, nonce });
                AdvertOutcome::Superseded
            }
        }
    }

    pub fn current(&self, key: NodeKey) -> Option<SocketAddr> {
        self.latest.get(&key).map(|a| a.reflexive)
    }

    /// Reverse-map an observed source back to the key that advertised it. Used
    /// by the coordinator to bind a caller's datagram source to its identity.
    pub fn key_for_src(&self, src: SocketAddr) -> Option<NodeKey> {
        self.latest
            .iter()
            .find(|(_, a)| a.reflexive == src)
            .map(|(k, _)| *k)
    }
}
```

- [ ] **Step 4: Wire the module into `lib.rs`**

Add near the other `pub mod` lines (advert is pure — always compiled):

```rust
pub mod advert;
```

Extend the crate-root re-exports:

```rust
pub use advert::{AdvertBook, AdvertOutcome, ReflexiveAdvert};
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal advert::`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/advert.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): monotonic-nonce reflexive AdvertBook (mirrors wireguard-upgrade dup rule)"
```

---

### Task 2: `SimNat::rebind` — model a reflexive-address change

**Files:**
- Modify: `crates/system/nat-traversal/src/simnat.rs`
- Test: inline `#[cfg(test)]` in `src/simnat.rs`

**Interfaces:**
- `pub fn SimNat::rebind(&mut self)` — model the NAT dropping and re-creating its mappings (lease change / device reboot / mapping timeout): clear the internal→public map(s) AND the opened holes, so the next `send` from the same internal socket allocates a **fresh, different** public port and the **old mapping no longer admits** any peer. `next_port` keeps advancing (never resets), so the new reflexive is guaranteed distinct. Works for both `Cone` and `Symmetric` modes.

- [ ] **Step 1: Write the failing tests (RED)**

Add to `simnat.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn rebind_moves_the_reflexive_and_invalidates_the_old_mapping() {
        let mut nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let peer = a([198, 51, 100, 2], 51820);

        let old = nat.send(internal, a([192, 0, 2, 1], 3478)); // reflexive toward coordinator
        let _ = nat.send(internal, peer); // punch a hole toward the peer
        assert!(nat.allow_inbound(old, peer), "hole toward peer is open pre-rebind");

        // The NAT rebinds: mapping + holes are dropped.
        nat.rebind();

        // The next STUN send yields a DIFFERENT reflexive (the stale mapping is
        // superseded), and the old mapping admits nobody anymore.
        let new = nat.send(internal, a([192, 0, 2, 1], 3478));
        assert_ne!(old, new, "rebind must move the reflexive to a fresh port");
        assert!(!nat.allow_inbound(old, peer), "the old mapping no longer admits the peer");
    }

    #[test]
    fn rebind_moves_the_reflexive_for_symmetric_too() {
        let mut nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let internal = a([192, 168, 1, 5], 51820);
        let coord = a([192, 0, 2, 1], 3478);
        let old = nat.send(internal, coord);
        nat.rebind();
        let new = nat.send(internal, coord);
        assert_ne!(old, new, "symmetric rebind also moves the coordinator-facing mapping");
    }
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal --features simnat simnat::`
Expected: FAIL — `rebind` not defined.

- [ ] **Step 3: Implement `rebind`**

Add to `impl SimNat`, after `symmetric`:

```rust
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
```

- [ ] **Step 4: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat simnat::`
Expected: PASS — 2 new tests + the 3 existing `simnat::` tests (cone/symmetric behavior otherwise unchanged).

- [ ] **Step 5: Commit**

```bash
git add crates/system/nat-traversal/src/simnat.rs
git commit -m "feat(nat-traversal): SimNat::rebind — model a reflexive-address change"
```

---

### Task 3: Coordinator adopts `AdvertBook` + nonce-gated `readvertise`

Swap the coordinator's ad-hoc `reflexive: HashMap<NodeKey, SocketAddr>` for the `AdvertBook` from Task 1 (behavior-preserving for `observe`/`current`/reverse-map) and add the nonce-gated `readvertise` path. This is what lets a peer re-resolve the superseding reflexive after a rebind.

**Files:**
- Modify: `crates/system/nat-traversal/src/coordinator.rs`
- Test: inline `#[cfg(test)]` in `src/coordinator.rs`

**Interfaces:**
- `Coordinator` field `reflexive` becomes `adverts: AdvertBook`.
- `Register` handler → `self.adverts.observe(key, from)`.
- `Lookup` handler → target via `self.adverts.current(key)`; caller key via `self.adverts.key_for_src(from)`.
- `request_relay` reverse-map → `self.adverts.key_for_src(caller_src)`.
- New: `pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome` — delegates to the book; the reachability-plane rebind supersede.

- [ ] **Step 1: Write the failing test (RED)**

Add to `coordinator.rs`'s `#[cfg(test)] mod tests`:

```rust
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
```

Add `use crate::AdvertOutcome;` to the test module's `use` lines if not already imported via `use super::*;` (it is re-exported at the crate root, so `super::*` may not surface it — import explicitly).

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal coordinator::`
Expected: FAIL — `readvertise` not defined.

- [ ] **Step 3: Swap the field to `AdvertBook`**

Update the top-of-file imports:

```rust
use std::collections::HashMap;
use std::net::SocketAddr;

use crate::advert::{AdvertBook, AdvertOutcome};
use crate::{Msg, NodeKey};
```

Replace the `reflexive` field in the struct:

```rust
#[derive(Default)]
pub struct Coordinator {
    adverts: AdvertBook,
    relay_by_pair: HashMap<(NodeKey, NodeKey), u64>,
    relay_sessions: HashMap<u64, RelaySession>,
    next_session: u64,
}
```

- [ ] **Step 4: Route `Register`/`Lookup`/`request_relay` through the book**

In `handle`, the `Register` arm:

```rust
            Msg::Register { key } => {
                // The registered reflexive address IS the observed source: the
                // coordinator never trusts a self-reported address.
                self.adverts.observe(key, from);
                Vec::new()
            }
```

The `Lookup` arm:

```rust
            Msg::Lookup { key } => {
                let target = self.adverts.current(key);
                let mut out = vec![(from, Msg::LookupResponse { key, reflexive: target })];
                if let Some(peer_addr) = target {
                    let caller_key = self.adverts.key_for_src(from).unwrap_or(NodeKey([0u8; 32]));
                    out.push((from, Msg::PunchSync { peer: key, peer_reflexive: peer_addr }));
                    out.push((peer_addr, Msg::PunchSync { peer: caller_key, peer_reflexive: from }));
                }
                out
            }
```

In `request_relay`, replace the reverse-map:

```rust
        let caller = self.adverts.key_for_src(caller_src)?;
```

- [ ] **Step 5: Add `readvertise`**

In `impl Coordinator`, next to `request_relay`:

```rust
    /// Reachability-plane rebind re-advertisement. A node whose NAT rebound
    /// re-runs STUN (its datagram is observed from a NEW source) and calls this
    /// under a strictly-higher `nonce` to supersede its stale reflexive; an
    /// equal-or-lower nonce is rejected as stale (a replay cannot clobber the
    /// fresh mapping). After a `Superseded`, a peer's `Lookup` resolves the new
    /// reflexive. This never touches relay/validator state.
    pub fn readvertise(&mut self, key: NodeKey, src: SocketAddr, nonce: u64) -> AdvertOutcome {
        self.adverts.readvertise(key, src, nonce)
    }
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p nat-traversal coordinator::`
Expected: PASS — the new test + all existing `coordinator::` tests (bind echo, register/lookup, relay allocation, release, prune) unchanged.

- [ ] **Step 7: Commit**

```bash
git add crates/system/nat-traversal/src/coordinator.rs
git commit -m "refactor(nat-traversal): coordinator reflexive registry -> AdvertBook + nonce-gated readvertise"
```

---

### Task 4: `drive_rebind_reconnect` — the deterministic rebind → re-resolve → reconnect proof

Factor the punch retry loop out of `drive_simulated` into a reusable `punch_until_bidirectional` (behavior-preserving), then add the end-to-end rebind driver that: hole-punches once, rebinds A's NAT, re-runs STUN, re-advertises under a higher nonce, has B re-resolve the new reflexive, and reconnects on the new mapping — proving the full path deterministically.

**Files:**
- Modify: `crates/system/nat-traversal/src/punch.rs` (`punch_until_bidirectional`, `drive_rebind_reconnect`, `RebindProof`)
- Modify: `crates/system/nat-traversal/src/lib.rs` (re-exports under the `simnat` gate)
- Test: inline `#[cfg(test)]` in `src/punch.rs`

**Interfaces:**
- `pub struct RebindProof { pub old_a_reflexive: SocketAddr, pub new_a_reflexive: SocketAddr, pub a_plan: PunchPlan, pub b_plan: PunchPlan }`.
- `pub fn drive_rebind_reconnect(a_key, b_key, a_nat: &mut SimNat, b_nat: &mut SimNat, coord: &mut Coordinator) -> Result<RebindProof, PunchError>`.
- private `fn punch_until_bidirectional(a: PunchSide, b: PunchSide, a_nat, b_nat) -> Result<(), PunchError>` (extracted; both `drive_simulated` and `drive_rebind_reconnect` call it).

- [ ] **Step 1: Write the failing test (RED)**

Add to `punch.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn rebind_then_reresolve_then_reconnect_on_the_new_mapping() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();

        let proof = drive_rebind_reconnect(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord)
            .expect("rebind reconnect");

        // The reflexive actually MOVED, and B re-resolved the NEW one.
        assert_ne!(proof.old_a_reflexive, proof.new_a_reflexive);
        assert_eq!(proof.a_plan.local_mapped, proof.new_a_reflexive);
        assert_eq!(
            proof.b_plan.peer_reflexive, proof.new_a_reflexive,
            "B reconnected against A's superseding reflexive, not the stale one"
        );
        // Bidirectional reachability re-established on the new mapping.
        assert!(a_nat.allow_inbound(proof.a_plan.local_mapped, proof.b_plan.local_mapped));
        assert!(b_nat.allow_inbound(proof.b_plan.local_mapped, proof.a_plan.local_mapped));
    }
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: FAIL — `drive_rebind_reconnect` / `RebindProof` not defined.

- [ ] **Step 3: Extract `punch_until_bidirectional` (behavior-preserving)**

Add below `punch_once`:

```rust
/// Drive simultaneous-open with retry until BOTH directions have had a datagram
/// actually admitted (observed per-round, not inferred from final filter
/// state), or the attempt budget is exhausted. Shared by `drive_simulated` and
/// `drive_rebind_reconnect`.
fn punch_until_bidirectional(
    a_side: PunchSide,
    b_side: PunchSide,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
) -> Result<(), PunchError> {
    let mut a_delivered = false;
    let mut b_delivered = false;
    for _ in 0..MAX_PUNCH_ATTEMPTS {
        if a_delivered && b_delivered {
            break;
        }
        let (a_ok, b_ok) = punch_once(a_side, b_side, a_nat, b_nat);
        a_delivered |= a_ok;
        b_delivered |= b_ok;
    }
    if !a_delivered || !b_delivered {
        return Err(PunchError::NotReachable);
    }
    Ok(())
}
```

Replace the inline retry loop + check in `drive_simulated` (steps 3–4 of that function) with a call:

```rust
    let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
    let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };
    punch_until_bidirectional(a_side, b_side, a_nat, b_nat)?;

    Ok((
        PunchPlan { local_mapped: a_mapped, peer_reflexive: a_peer },
        PunchPlan { local_mapped: b_mapped, peer_reflexive: b_peer },
    ))
```

- [ ] **Step 4: Implement `drive_rebind_reconnect` + `RebindProof`**

Add below `drive_simulated` (and above `internal`):

```rust
/// The proof a rebind reconnect produces: the reflexive before and after the
/// rebind (they must differ), plus the fresh punch plans on the new mapping.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RebindProof {
    pub old_a_reflexive: SocketAddr,
    pub new_a_reflexive: SocketAddr,
    pub a_plan: PunchPlan,
    pub b_plan: PunchPlan,
}

/// Deterministic full rebinding path: punch once, then A's NAT rebinds, A
/// re-runs STUN and re-advertises under a HIGHER monotonic nonce (superseding
/// its stale mapping), B re-resolves the new reflexive via the coordinator, and
/// the pair reconnects on the new mapping. This is the CI proof for
/// "endpoint-churn re-advertisement" (Acceptance §1) and the design's
/// "NAT rebinding → re-run STUN and re-advertise under a higher monotonic
/// nonce" fallback rule.
pub fn drive_rebind_reconnect(
    a_key: NodeKey,
    b_key: NodeKey,
    a_nat: &mut SimNat,
    b_nat: &mut SimNat,
    coord: &mut Coordinator,
) -> Result<RebindProof, PunchError> {
    // 1. Establish the initial direct path (also registers both nodes).
    let (a_plan0, _b_plan0) = drive_simulated(a_key, b_key, a_nat, b_nat, coord)?;
    let old_a_reflexive = a_plan0.local_mapped;

    // 2. A's NAT rebinds: its mappings + holes are dropped.
    a_nat.rebind();

    // 3. A re-runs STUN: the datagram traverses the rebound NAT to a FRESH
    //    reflexive, which the coordinator observes.
    let new_a_mapped = a_nat.send(internal(&a_key), coord_addr());

    // 4. A re-advertises under a strictly-higher nonce; it must supersede.
    if coord.readvertise(a_key, new_a_mapped, 1) != crate::AdvertOutcome::Superseded {
        return Err(PunchError::NoReflexive);
    }

    // 5. B re-resolves: its Lookup now returns A's NEW reflexive, and the
    //    coordinator fans out PunchSync to both the new A mapping and B.
    let b_mapped = a_plan0_b_mapped(b_key, b_nat);
    let out = coord.handle(b_mapped, Msg::Lookup { key: a_key });
    let mut b_peer = None; // A's new reflexive, as B sees it
    let mut a_peer = None; // B's reflexive, as A sees it (via the fan-out)
    for (dst, msg) in out {
        if let Msg::PunchSync { peer_reflexive, .. } = msg {
            if dst == b_mapped {
                b_peer = Some(peer_reflexive);
            } else if dst == new_a_mapped {
                a_peer = Some(peer_reflexive);
            }
        }
    }
    let b_peer = b_peer.ok_or(PunchError::NoReflexive)?;
    let a_peer = a_peer.ok_or(PunchError::NoReflexive)?;

    // 6. Reconnect on the new mapping.
    let a_side = PunchSide { key: a_key, mapped: new_a_mapped, peer: a_peer };
    let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };
    punch_until_bidirectional(a_side, b_side, a_nat, b_nat)?;

    Ok(RebindProof {
        old_a_reflexive,
        new_a_reflexive: new_a_mapped,
        a_plan: PunchPlan { local_mapped: new_a_mapped, peer_reflexive: a_peer },
        b_plan: PunchPlan { local_mapped: b_mapped, peer_reflexive: b_peer },
    })
}

// B's coordinator-facing mapping is stable (B did not rebind), so re-deriving it
// is idempotent and matches what registration observed.
fn a_plan0_b_mapped(b_key: NodeKey, b_nat: &mut SimNat) -> SocketAddr {
    b_nat.send(internal(&b_key), coord_addr())
}
```

- [ ] **Step 5: Re-export under the `simnat` gate in `lib.rs`**

Extend the existing feature-gated `pub use punch::{...}`:

```rust
#[cfg(any(test, feature = "simnat"))]
pub use punch::{
    FallbackOutcome, PunchError, PunchPlan, RebindProof, RelayFallbackProof, drive_rebind_reconnect,
    drive_simulated, drive_with_relay_fallback,
};
```

- [ ] **Step 6: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: PASS — the new rebind test + all existing `punch::` tests (the `punch_until_bidirectional` extraction is behavior-preserving, so `two_hidden_endpoints_punch_through_restricted_cone`, the symmetric-fail, and the relay-fallback tests stay green).

- [ ] **Step 7: Commit**

```bash
git add crates/system/nat-traversal/src/punch.rs crates/system/nat-traversal/src/lib.rs
git commit -m "feat(nat-traversal): drive_rebind_reconnect — deterministic rebind->re-resolve->reconnect"
```

---

### Task 5: Multiple coordinators — `NatClient` failover across a hint `Vec`

The reach-hint set is a `Vec`; a joiner tries coordinators in order and falls through a dead/unresponsive one within a bounded budget to the next, never getting stuck. No single coordinator is uniquely load-bearing.

**Files:**
- Modify: `crates/system/nat-traversal/src/client.rs` (`bind_multi`, `coords` field, `discover_reflexive_failover`)
- Test: inline `#[cfg(test)]` async tests in `src/client.rs`

**Interfaces:**
- `NatClient` gains a `coords: Vec<SocketAddr>` field (the ordered hint set). `bind` keeps its single-address signature and sets `coords = vec![coord]`; `bind_multi(key, coords: Vec<SocketAddr>)` takes the full set (primary = `coords[0]`).
- `pub async fn discover_reflexive_failover(&self, per_try: Duration) -> io::Result<(usize, SocketAddr)>` — tries each coordinator in order, waiting at most `per_try` for a `BindResponse` from *that* coordinator; on timeout/no-response, falls through to the next. Returns `(index, reflexive)` of the coordinator that answered. Bounded by `per_try * coords.len()`; never blocks forever. Errors only if every coordinator is silent.

- [ ] **Step 1: Write the failing tests (RED)**

Add to `client.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[tokio::test]
    async fn dead_primary_falls_through_to_live_secondary() {
        // A live coordinator (the secondary).
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live));

        // A DEAD primary: a bound socket nobody ever serves. Datagrams sent to
        // it are buffered and never answered, so the per-try budget elapses.
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let client = NatClient::bind_multi(NodeKey([1u8; 32]), vec![dead_addr, live_addr])
            .await
            .unwrap();
        let (idx, reflexive) =
            timeout(Duration::from_secs(2), client.discover_reflexive_failover(Duration::from_millis(150)))
                .await
                .expect("failover must be bounded, never stuck")
                .expect("secondary answers");

        assert_eq!(idx, 1, "the dead primary is skipped; the live secondary answers");
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }

    #[tokio::test]
    async fn no_single_coordinator_is_load_bearing_either_position_works() {
        // Same live coordinator, but now in PRIMARY position with a dead
        // secondary: discovery still succeeds, via index 0. Together with the
        // previous test this proves neither position is uniquely required.
        let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let live_addr = live.local_addr().unwrap();
        tokio::spawn(run_coordinator(live));
        let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();

        let client = NatClient::bind_multi(NodeKey([2u8; 32]), vec![live_addr, dead_addr])
            .await
            .unwrap();
        let (idx, reflexive) =
            timeout(Duration::from_secs(2), client.discover_reflexive_failover(Duration::from_millis(150)))
                .await
                .expect("no timeout")
                .expect("primary answers");
        assert_eq!(idx, 0, "a live primary is used directly");
        assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
    }
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal client::`
Expected: FAIL — `bind_multi` / `discover_reflexive_failover` not defined.

- [ ] **Step 3: Add the `coords` field + `bind_multi`**

Update the struct and constructors:

```rust
pub struct NatClient {
    sock: UdpSocket,
    key: NodeKey,
    coord: SocketAddr,
    coords: Vec<SocketAddr>,
}

impl NatClient {
    pub async fn bind(key: NodeKey, coord: SocketAddr) -> std::io::Result<Self> {
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords: vec![coord] })
    }

    /// Bind with an ordered set of coordinator hints (the reach `Vec`). The
    /// primary is `coords[0]`; single-coordinator methods use it, while
    /// `discover_reflexive_failover` walks the whole set.
    pub async fn bind_multi(key: NodeKey, coords: Vec<SocketAddr>) -> std::io::Result<Self> {
        let coord = *coords.first().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "empty coordinator set")
        })?;
        let sock = UdpSocket::bind("0.0.0.0:0").await?;
        Ok(Self { sock, key, coord, coords })
    }
```

(Leave every existing method that uses `self.coord` unchanged — the primary stays the default target.)

- [ ] **Step 4: Add `discover_reflexive_failover`**

In `impl NatClient`, next to `discover_reflexive`:

```rust
    /// Discover this node's reflexive address, trying each coordinator hint in
    /// order and falling through a dead/unresponsive one after `per_try` to the
    /// next. Returns the index of the coordinator that answered plus the
    /// reflexive it observed. Total wait is bounded by `per_try * coords.len()`,
    /// so a dead coordinator never wedges the joiner — the coordinator set is
    /// not uniquely load-bearing.
    pub async fn discover_reflexive_failover(
        &self,
        per_try: std::time::Duration,
    ) -> std::io::Result<(usize, SocketAddr)> {
        for (i, &c) in self.coords.iter().enumerate() {
            self.sock
                .send_to(&Msg::BindRequest { from: self.key }.encode(), c)
                .await?;
            let attempt = async {
                let mut buf = [0u8; 64];
                loop {
                    let (n, from) = self.sock.recv_from(&mut buf).await?;
                    // Only THIS coordinator's own reply counts; a stray/forged
                    // datagram from anyone else is ignored (same rule as the
                    // single-coordinator discover_reflexive).
                    if from != c {
                        continue;
                    }
                    if let Ok(Msg::BindResponse { reflexive }) = Msg::decode(&buf[..n]) {
                        return Ok::<SocketAddr, std::io::Error>(reflexive);
                    }
                }
            };
            match tokio::time::timeout(per_try, attempt).await {
                Ok(Ok(reflexive)) => return Ok((i, reflexive)),
                // Timeout or socket error on this coordinator -> try the next.
                _ => continue,
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "no coordinator in the hint set responded",
        ))
    }
```

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal client::`
Expected: PASS — both new tests + all existing `client::` tests.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/client.rs
git commit -m "feat(nat-traversal): NatClient coordinator failover across the reach-hint Vec"
```

---

### Task 6: Keepalive / survival semantics — punched survives, relayed does not

Encode the asymmetry precisely: a PUNCHED direct path is coordinator-independent after setup; a RELAYED path's data forwarding lives in the coordinator process and its (re)establishment requires a live coordinator. **Do not overclaim.** The Slice 2 idle-prune (reclaim-on-splice-completion in `run_coordinator_with_idle`) is the wall-clock prune and stays as is — this task adds tests around it plus a small `relay_session_count` accessor to make the bound observable.

**Files:**
- Modify: `crates/system/nat-traversal/src/punch.rs` (deterministic survival tests + a `coord_ip()` helper if useful)
- Modify: `crates/system/nat-traversal/src/coordinator.rs` (`relay_session_count` accessor + a bound test)
- Modify: `crates/system/nat-traversal/src/client.rs` (async survival tests)

- [ ] **Step 1: Write the failing tests (RED)**

Add to `punch.rs`'s `#[cfg(test)] mod tests` (deterministic asymmetry):

```rust
    #[test]
    fn punched_direct_path_survives_coordinator_going_away() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();
        let (a_plan, b_plan) =
            drive_simulated(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord).expect("punch");

        // The coordinator is gone. A direct, punched path lives entirely in the
        // two NAT filter states — nothing here consults the coordinator.
        drop(coord);
        assert!(a_nat.allow_inbound(a_plan.local_mapped, b_plan.local_mapped));
        assert!(b_nat.allow_inbound(b_plan.local_mapped, a_plan.local_mapped));
    }

    #[test]
    fn relayed_path_rides_the_coordinator_unlike_a_punched_one() {
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::symmetric(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();
        let outcome = drive_with_relay_fallback(
            a_key, b_key, &mut a_nat, &mut b_nat, &mut coord, b"a", b"b",
        )
        .expect("relay");
        let coord_ip = coord_addr().ip();
        match outcome {
            FallbackOutcome::Relayed(p) => {
                // Both relay endpoints sit ON the coordinator: the data path
                // traverses it, so coordinator death kills the relayed path.
                assert_eq!(p.a_relay_endpoint.ip(), coord_ip);
                assert_eq!(p.b_relay_endpoint.ip(), coord_ip);
            }
            FallbackOutcome::Punched { .. } => panic!("symmetric pair must relay"),
        }
    }
```

Add to `coordinator.rs`'s `#[cfg(test)] mod tests` (relay-state bound):

```rust
    #[test]
    fn prune_relays_returns_state_to_zero_bounded() {
        let mut c = Coordinator::new();
        let a_src = addr(1, 1111);
        let a = NodeKey([0xaa; 32]);
        let b = NodeKey([0xbb; 32]);
        c.handle(a_src, Msg::Register { key: a });
        let _ = c.request_relay(a_src, b, 0).expect("session");
        assert_eq!(c.relay_session_count(), 1);
        c.prune_relays(100, 10);
        assert_eq!(c.relay_session_count(), 0, "idle prune bounds relay state");
    }
```

Add to `client.rs`'s `#[cfg(test)] mod tests` (async survival asymmetry):

```rust
    #[tokio::test]
    async fn direct_path_survives_coordinator_shutdown() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(coord_sock));

        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let a = NatClient::bind(a_key, coord_addr).await.unwrap();
        let b = NatClient::bind(b_key, coord_addr).await.unwrap();
        a.register().await.unwrap();
        b.register().await.unwrap();

        // Rendezvous via the coordinator to learn each other's addresses.
        let _b_reflexive = timeout(Duration::from_secs(2), a.lookup(b_key))
            .await
            .expect("no timeout")
            .expect("lookup");
        let b_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            b.local_addr().await.unwrap().port(),
        );
        let a_addr = SocketAddr::new(
            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            a.local_addr().await.unwrap().port(),
        );

        // The coordinator dies.
        coord.abort();

        // The direct path still works: A sends straight to B, no coordinator.
        a.send_punch_to(b_addr).await.unwrap();
        let got = timeout(Duration::from_secs(2), b.recv_punch_from(a_addr))
            .await
            .expect("direct path must survive coordinator downtime")
            .expect("recv");
        assert_eq!(got, Msg::Punch { from: a_key });
    }

    #[tokio::test]
    async fn relay_setup_requires_a_live_coordinator() {
        let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let coord_addr = coord_sock.local_addr().unwrap();
        let coord = tokio::spawn(run_coordinator(coord_sock));

        let a = NatClient::bind(NodeKey([0xaa; 32]), coord_addr).await.unwrap();
        a.register().await.unwrap();

        // Coordinator down -> a relayed path cannot even be established: the
        // grant never comes. (Unlike a punched path, which needs nothing.)
        coord.abort();
        let res = timeout(Duration::from_millis(400), a.request_relay(NodeKey([0xbb; 32]))).await;
        assert!(
            res.is_err(),
            "without a live coordinator a relay session cannot be allocated"
        );
    }
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p nat-traversal --features simnat survives; cargo test -p nat-traversal --features simnat relay_session; cargo test -p nat-traversal relay_setup`
Expected: FAIL — `relay_session_count` not defined; the survival tests reference it and the `coord_addr()`/`FallbackOutcome` already exist in `punch.rs`.

- [ ] **Step 3: Add `relay_session_count`**

In `impl Coordinator`:

```rust
    /// Number of live relay sessions. Lets tests assert the idle prune keeps
    /// relay state bounded. (Public; not dead code in a plain build.)
    pub fn relay_session_count(&self) -> usize {
        self.relay_sessions.len()
    }
```

- [ ] **Step 4: Confirm the survival tests compile against existing API**

The deterministic tests use `drive_simulated`, `drive_with_relay_fallback`, `FallbackOutcome`, and `coord_addr()` (already private in `punch.rs`, so the tests — same module — can call it). The async tests use `run_coordinator`, `NatClient::{bind,register,lookup,send_punch_to,recv_punch_from,request_relay}` and `JoinHandle::abort` (from `tokio::spawn`). No new library code beyond `relay_session_count` is needed.

- [ ] **Step 5: Run to verify pass**

Run: `cargo test -p nat-traversal --features simnat`
Expected: PASS — all new survival/bound tests plus the full existing suite.

- [ ] **Step 6: Commit**

```bash
git add crates/system/nat-traversal/src/punch.rs crates/system/nat-traversal/src/coordinator.rs crates/system/nat-traversal/src/client.rs
git commit -m "test(nat-traversal): survival asymmetry — punched survives, relayed needs a live coordinator"
```

---

### Task 7: Strengthen the hole-punch proof — bidirectional DELIVERY under adverse interleaving (Slice 0a carry-over)

Slice 0a's review carried forward: the headline hole-punch proof must assert **actual bidirectional delivery** observed **per datagram under an adverse send order** (A sends before B has opened its filter), not merely the aggregate final NAT-filter state. `drive_simulated` already checks per-round delivery via `punch_once`; this task adds the explicit adverse-interleave proof that would fail if the driver ever regressed to a final-state-only check.

**Files:**
- Modify: `crates/system/nat-traversal/src/punch.rs`
- Test: inline `#[cfg(test)]` in `src/punch.rs`

- [ ] **Step 1: Write the failing test (RED)**

Add to `punch.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    fn adverse_interleave_first_datagram_drops_but_retry_delivers_both_directions() {
        // The adverse case: A's punch is sent strictly BEFORE B has opened its
        // filter toward A (fixed send order in `punch_once`). A final-state-only
        // check would see both filters eventually open and wrongly call it a
        // success on round 1; the real proof observes each datagram AT SEND TIME.
        let a_key = NodeKey([0xaa; 32]);
        let b_key = NodeKey([0xbb; 32]);
        let mut a_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord = Coordinator::new();
        let (a_mapped, b_mapped, a_peer, b_peer) =
            rendezvous(a_key, b_key, &mut a_nat, &mut b_nat, &mut coord);
        let a_side = PunchSide { key: a_key, mapped: a_mapped, peer: a_peer };
        let b_side = PunchSide { key: b_key, mapped: b_mapped, peer: b_peer };

        // Round 1 (adverse): A's datagram is DROPPED (B's filter not yet open);
        // B's lands (A opened its filter first this round).
        let (a1, b1) = punch_once(a_side, b_side, &mut a_nat, &mut b_nat);
        assert!(!a1, "A's FIRST punch is dropped under the adverse order");
        assert!(b1, "B's punch is delivered because A already opened its filter");

        // Round 2 (retry): A's retransmit is now admitted — B opened its filter
        // in round 1. BOTH directions have now had a datagram actually delivered,
        // observed per-round, not inferred from final filter state.
        let (a2, _b2) = punch_once(a_side, b_side, &mut a_nat, &mut b_nat);
        assert!(a2, "A's retransmit is delivered on round 2: real bidirectional delivery");

        // And the full driver reaches the same success on the same fresh pair.
        let mut a_nat2 = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 1)));
        let mut b_nat2 = SimNat::new(IpAddr::V4(Ipv4Addr::new(198, 51, 100, 2)));
        let mut coord2 = Coordinator::new();
        drive_simulated(a_key, b_key, &mut a_nat2, &mut b_nat2, &mut coord2)
            .expect("driver delivers both directions despite the adverse first-drop");
    }
```

- [ ] **Step 2: Run to confirm it fails, then pass**

Run: `cargo test -p nat-traversal --features simnat punch::adverse_interleave`
Expected: initially FAIL only if the test name/refs are wrong; since `punch_once`, `rendezvous`, `PunchSide`, and `drive_simulated` all already exist, this is a *strengthening* test that should pass immediately against the current (correct) implementation. Its value is as a regression lock: it FAILS if `drive_simulated`/`punch_once` ever regress to a final-state-only check. Confirm it passes.

Run: `cargo test -p nat-traversal --features simnat punch::`
Expected: PASS — new test + existing `punch::` suite.

- [ ] **Step 3: Commit**

```bash
git add crates/system/nat-traversal/src/punch.rs
git commit -m "test(nat-traversal): lock bidirectional delivery under adverse punch interleaving (Slice 0a carry-over)"
```

---

### Task 8: Consolidate — the CI simulated-NAT suite (merge gate)

Gather the in-scope Acceptance §1 proofs into one integration test that exercises only the crate's public API under `--features simnat`, each item a named test, with a module doc comment mapping 1:1 to the design's Acceptance §1 list (and recording that v3-invite/v2-parse live in `bin/node`, Slice 1, out of this gate). This is the reviewable merge-gate artifact.

**Files:**
- Create: `crates/system/nat-traversal/tests/simnat_ci.rs`

- [ ] **Step 1: Write the suite (it must pass against the public API built in Tasks 1–7)**

`crates/system/nat-traversal/tests/simnat_ci.rs`:

```rust
//! CI simulated-NAT suite — the private-cutover epic merge gate.
//!
//! Maps 1:1 to `docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`
//! §"Acceptance" item 1. Run with `--features simnat`.
//!
//! | Acceptance §1 item                       | Test here                                   |
//! |------------------------------------------|---------------------------------------------|
//! | reflexive discovery                      | `reflexive_discovery`                       |
//! | hole-punch success                       | `hole_punch_success`                        |
//! | hole-punch failure -> relay splice       | `hole_punch_failure_relays_bidirectionally` |
//! | endpoint-churn re-advertisement          | `endpoint_churn_readvertise_reconnect`      |
//! | (multiple coordinators — Slice 3)        | `multi_coordinator_failover`                |
//! | (keepalive survival — Slice 3)           | `punched_survives_relayed_needs_coordinator`|
//!
//! Out of THIS gate (node-bin's clippy is pre-existingly red from toolchain
//! drift): v3 invite signature verify/reject and v2 parse-compatibility live in
//! `bin/node/src/config.rs` and are covered by Slice 1's own tests.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use nat_traversal::{
    Coordinator, FallbackOutcome, NatClient, NodeKey, SimNat, drive_rebind_reconnect,
    drive_simulated, drive_with_relay_fallback, run_coordinator,
};
use tokio::net::UdpSocket;
use tokio::time::timeout;

fn ip(o: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(198, 51, 100, o))
}

#[tokio::test]
async fn reflexive_discovery() {
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    tokio::spawn(run_coordinator(coord_sock));
    let client = NatClient::bind(NodeKey([1u8; 32]), coord_addr).await.unwrap();
    let reflexive = timeout(Duration::from_secs(2), client.discover_reflexive())
        .await
        .expect("no timeout")
        .expect("reflexive");
    assert_eq!(reflexive.port(), client.local_addr().await.unwrap().port());
}

#[test]
fn hole_punch_success() {
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::new(ip(1));
    let mut b_nat = SimNat::new(ip(2));
    let mut coord = Coordinator::new();
    let (ap, bp) = drive_simulated(a, b, &mut a_nat, &mut b_nat, &mut coord).expect("punch");
    assert_eq!(ap.peer_reflexive, bp.local_mapped);
    assert_eq!(bp.peer_reflexive, ap.local_mapped);
}

#[test]
fn hole_punch_failure_relays_bidirectionally() {
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::symmetric(ip(1));
    let mut b_nat = SimNat::symmetric(ip(2));
    let mut coord = Coordinator::new();
    let outcome =
        drive_with_relay_fallback(a, b, &mut a_nat, &mut b_nat, &mut coord, b"ping", b"pong")
            .expect("relay");
    match outcome {
        FallbackOutcome::Relayed(p) => {
            assert_eq!(p.delivered_to_b, b"ping");
            assert_eq!(p.delivered_to_a, b"pong");
            assert_ne!(p.a_relay_endpoint, p.b_relay_endpoint);
        }
        FallbackOutcome::Punched { .. } => panic!("symmetric pair must relay"),
    }
}

#[test]
fn endpoint_churn_readvertise_reconnect() {
    let a = NodeKey([0xaa; 32]);
    let b = NodeKey([0xbb; 32]);
    let mut a_nat = SimNat::new(ip(1));
    let mut b_nat = SimNat::new(ip(2));
    let mut coord = Coordinator::new();
    let proof = drive_rebind_reconnect(a, b, &mut a_nat, &mut b_nat, &mut coord).expect("rebind");
    assert_ne!(proof.old_a_reflexive, proof.new_a_reflexive);
    assert_eq!(proof.b_plan.peer_reflexive, proof.new_a_reflexive);
}

#[tokio::test]
async fn multi_coordinator_failover() {
    let live = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let live_addr = live.local_addr().unwrap();
    tokio::spawn(run_coordinator(live));
    let dead = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let dead_addr = dead.local_addr().unwrap();

    let client = NatClient::bind_multi(NodeKey([3u8; 32]), vec![dead_addr, live_addr])
        .await
        .unwrap();
    let (idx, _reflexive) = timeout(
        Duration::from_secs(2),
        client.discover_reflexive_failover(Duration::from_millis(150)),
    )
    .await
    .expect("bounded")
    .expect("secondary answers");
    assert_eq!(idx, 1);
}

#[tokio::test]
async fn punched_survives_relayed_needs_coordinator() {
    // Punched path survives coordinator downtime (deterministic proof mirrored
    // here via the async direct send).
    let coord_sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let coord_addr = coord_sock.local_addr().unwrap();
    let coord = tokio::spawn(run_coordinator(coord_sock));
    let a = NatClient::bind(NodeKey([0xaa; 32]), coord_addr).await.unwrap();
    let b = NatClient::bind(NodeKey([0xbb; 32]), coord_addr).await.unwrap();
    a.register().await.unwrap();
    b.register().await.unwrap();
    let _ = timeout(Duration::from_secs(2), a.lookup(NodeKey([0xbb; 32])))
        .await
        .expect("no timeout")
        .expect("lookup");
    let b_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        b.local_addr().await.unwrap().port(),
    );
    let a_addr = SocketAddr::new(
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
        a.local_addr().await.unwrap().port(),
    );
    coord.abort();
    a.send_punch_to(b_addr).await.unwrap();
    let got = timeout(Duration::from_secs(2), b.recv_punch_from(a_addr))
        .await
        .expect("direct path survives")
        .expect("recv");
    assert_eq!(got, nat_traversal::Msg::Punch { from: NodeKey([0xaa; 32]) });

    // Relayed path needs a live coordinator: with it down, allocation fails.
    let c2 = NatClient::bind(NodeKey([0xcc; 32]), coord_addr).await.unwrap();
    let res = timeout(Duration::from_millis(400), c2.request_relay(NodeKey([0xdd; 32]))).await;
    assert!(res.is_err(), "relay setup requires a live coordinator");
}
```

- [ ] **Step 2: Run the suite**

Run: `cargo test -p nat-traversal --features simnat --test simnat_ci`
Expected: PASS — 6 tests (`reflexive_discovery`, `hole_punch_success`, `hole_punch_failure_relays_bidirectionally`, `endpoint_churn_readvertise_reconnect`, `multi_coordinator_failover`, `punched_survives_relayed_needs_coordinator`).

- [ ] **Step 3: Run the FULL merge gate**

```bash
cargo test -p nat-traversal --features simnat && \
cargo clippy -p nat-traversal --features simnat --all-targets -- -D warnings
```
Expected: all unit tests + `tests/punch_e2e.rs` + `tests/simnat_ci.rs` green; clippy clean with `-D warnings`. Fix any clippy nits (e.g. `needless_return`, `clippy::too_many_arguments` — `drive_with_relay_fallback` already carries the allow; add `#[allow(clippy::too_many_arguments)]` to any new 8+ arg fn if introduced) before committing.

- [ ] **Step 4: Commit**

```bash
git add crates/system/nat-traversal/tests/simnat_ci.rs
git commit -m "test(nat-traversal): consolidate the CI simulated-NAT suite (merge gate, Acceptance §1)"
```

---

## What this slice deliberately does NOT do

- **No `wireguard-upgrade` dependency.** The monotonic-nonce rule is modeled locally in `advert.rs` and only *mirrors* `MeshView::verify`. Coupling the crates would drag validator-identity types into the reachability plane and break the Slice 2 invariant.
- **No blind wall-clock relay prune.** Slice 2's reclaim-on-splice-completion is the idle prune; adding a competing periodic prune risks tearing down live sessions the coordinator can't observe. Task 6 only *tests* the existing bound.
- **No overclaim of relayed survival.** The tests assert exactly: punched paths are coordinator-independent post-setup; relayed paths are coordinator-owned (data endpoints on the coordinator) and cannot be (re)established without a live coordinator. They do not claim an already-forwarding splice keeps running when only the control loop is aborted (tokio child tasks outlive an aborted parent — an artifact of the test harness, not the real single-process coordinator).
- **No v3-invite / v2-parse coverage in this gate.** Those live in `bin/node/src/config.rs` (Slice 1) behind node-bin's pre-red clippy; Task 8's suite references them and delegates to Slice 1.
- **No cross-machine / real-WireGuard work.** Acceptance §2 (cross-machine zero-exposure demo) and §3 (real `p2p.ducktape` deployment) are Slice 4.

## Self-review notes

- **Gate scope honored:** every command is `-p nat-traversal`; the merge gate is `cargo test -p nat-traversal --features simnat && cargo clippy -p nat-traversal --features simnat --all-targets -- -D warnings`. `bin/node`/`noded` are never pulled.
- **Type consistency:** `Coordinator`, `SimNat`, `NatClient`, `NodeKey`, `Msg`, `PunchPlan`, `PunchError`, `FallbackOutcome`, `Side`, and the new `AdvertBook`/`AdvertOutcome`/`ReflexiveAdvert`/`RebindProof` names are used identically across tasks; `readvertise` returns `AdvertOutcome`; `drive_rebind_reconnect`/`discover_reflexive_failover` signatures match their test usage; the `simnat`-gated re-exports in `lib.rs` list `RebindProof`/`drive_rebind_reconnect` so `tests/simnat_ci.rs` (a `--features simnat` build) can import them.
- **Behavior-preserving refactors:** Task 3 (`AdvertBook` adoption) and Task 4 (`punch_until_bidirectional` extraction) keep the 31 existing unit tests + `tests/punch_e2e.rs` green with no assertion edits — verified by re-running the relevant subset each task.
- **No placeholders:** every step carries real, compile-ready code and an exact `cargo` command with its expected result. Baseline confirmed green before writing (31 unit + 1 integration test, clippy clean).
- **Requirement coverage:** (1) rebinding → Tasks 1–4; (2) multiple coordinators → Task 5; (3) keepalive/survival → Task 6; (4) full CI suite incl. Slice 0a carry-over → Tasks 7–8.
