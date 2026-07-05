# Slice 0b — WireGuard Effect Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the inert `wireguard-upgrade` protocol crate its first real consumer. Today `validate_upgrade` produces a `TunnelInstallPlan` that converts into defguard `InterfaceConfiguration`/`Peer` values, but nothing in the workspace ever calls `WGApi::configure_interface` — and the plan it produces is hard-wired to the initiator's perspective, so a responder cannot derive its own install config from the same validated handshake. This slice closes both gaps: a new `crates/system/wireguard-effect` crate provides a `WireGuardEffect` trait (a deterministic fake for CI, a real `defguard_wireguard_rs` adapter for cross-machine runs) plus a wiring function that applies a plan with a punch/relay-resolved peer endpoint override; and `wireguard-upgrade` gains an additive `validate_upgrade_as(Perspective, ...)` entry point so either party can derive its own plan from the identical signed triple.

**Architecture:** `crates/system/wireguard-effect` is a new leaf crate, one level above `wireguard-upgrade` in the dependency graph (`wireguard-effect` depends on `wireguard-upgrade`, never the reverse). It defines `WireGuardEffect` (create/apply/remove against a `defguard_wireguard_rs::InterfaceConfiguration`), `FakeWireGuardEffect` (records everything, used by every automated test — CI has no WireGuard userspace runtime), `DefguardWireGuardEffect` (wraps `WGApi::<Userspace>`, exercised only by an `#[ignore]`d manual cross-machine test), and `apply_tunnel_plan` (takes a validated `TunnelInstallPlan` + an optional peer-endpoint override and drives a `WireGuardEffect`). `wireguard-upgrade` itself gets one additive change: a `Perspective` enum and `validate_upgrade_as(perspective, ...)`, with `validate_upgrade` becoming a thin, behavior-identical wrapper around `validate_upgrade_as(Perspective::Initiator, ...)`. No existing public signature changes; no existing test's assertions change.

**Tech Stack:** Rust (edition 2024), `defguard_wireguard_rs = "0.10.0"` (already a workspace dependency, used today only by `wireguard-upgrade`'s pure-data conversions — verified against the vendored crate source at `~/.cargo/registry/src/index.crates.io-*/defguard_wireguard_rs-0.10.0`), `commonware-cryptography` (dev-only, for building signed test fixtures).

## Verified `defguard_wireguard_rs` 0.10 surface (read from the vendored crate source, not from memory)

- `WGApi<API = Kernel>` (`src/wgapi.rs`): `pub fn new<S: Into<String>>(ifname: S) -> Result<Self, WireguardInterfaceError>` — pure struct construction (stores `ifname`, `device_handle: None`); does **not** touch the network or require privilege unless the crate's `check_dependencies` feature is enabled (this workspace does not enable it). Safe to call in CI.
- `WGApi<Userspace>` implements `WireguardInterfaceApi` (`src/wgapi_userspace.rs`, `#[cfg(unix)]`-gated at the module level in `src/lib.rs`):
  - `fn create_interface(&mut self) -> Result<(), WireguardInterfaceError>` — spawns a BoringTun `DeviceHandle`. Requires a real, privileged (root/`CAP_NET_ADMIN`) host.
  - `fn configure_interface(&self, config: &InterfaceConfiguration) -> Result<(), WireguardInterfaceError>` — writes the host config over a UNIX socket at `/var/run/wireguard/<ifname>.sock`.
  - `fn remove_interface(&self) -> Result<(), WireguardInterfaceError>`.
  - None of these three are reachable in CI (no privileged host, no BoringTun socket) — this is why `FakeWireGuardEffect` is the only implementation the automated suite exercises.
- `InterfaceConfiguration` (`src/lib.rs`): `{ name: String, prvkey: String, addresses: Vec<IpAddrMask>, port: u16, peers: Vec<Peer>, mtu: Option<u32>, fwmark: Option<u32> }`. Derives `Clone` only (manual `Debug`, no `PartialEq` — tests must assert on individual fields, not whole-struct equality).
- `Peer` (`src/peer.rs`): `{ public_key: Key, preshared_key: Option<Key>, endpoint: Option<SocketAddr>, persistent_keepalive_interval: Option<u16>, allowed_ips: Vec<IpAddrMask>, .. }`. Derives `Clone, Default, PartialEq` (+ manual `Debug`) — `Key` has a hand-written `PartialEq`, so whole-`Peer` equality checks work.
- `IpAddrMask` (`src/net.rs`): `{ address: IpAddr, cidr: u8 }`, `pub fn new(address: IpAddr, cidr: u8) -> Self`. Derives `Clone, Debug, Eq, Hash, PartialEq`.
- `Key` (`src/key.rs`): `pub fn new(buf: [u8; 32]) -> Self`, `pub fn as_array(&self) -> [u8; 32]`.
- `error::WireguardInterfaceError`: `#[derive(Debug, Error)] #[non_exhaustive]`.
- Re-exports at crate root: `pub use wgapi::{Kernel, Userspace, WGApi}; pub use wireguard_interface::WireguardInterfaceApi;`, plus `pub mod key; pub mod net; pub mod peer; pub mod error;` — matching exactly what `wireguard-upgrade/src/lib.rs` already imports (`InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer`).

## Verified gap in `wireguard-upgrade` (read from `src/lib.rs` and `tests/tunnel_e2e.rs`, not from memory)

`validate_upgrade` (currently `crates/system/wireguard-upgrade/src/lib.rs:823-964`) runs the full handshake validation and then constructs exactly one `TunnelInstallPlan`, hard-coded to the initiator's side:

```rust
Ok(TunnelInstallPlan {
    // ...
    local_identity: rq.initiator_identity,
    peer_identity: rq.responder_identity,
    local_wireguard_public_key: rq.initiator_wireguard_public_key,
    peer_wireguard_public_key: rs.responder_wireguard_public_key,
    peer_endpoint: rs.responder_wireguard_endpoint,
    local_interface_ips: rs.accepted_allowed_ips.clone(),
    allowed_ips: rq.requested_allowed_ips.clone(),
    relay_candidates: rs.relay_candidates.clone(),
    keepalive_seconds: rs.keepalive_seconds,
})
```

`tests/tunnel_e2e.rs::both_parties_validate_but_the_plan_is_initiator_local` pins this: calling `validate_upgrade` from the responder's own `MeshView` still returns a plan whose `local_identity()` is the **initiator**, because `TunnelInstallPlan` has no responder-side constructor — its fields are all private and it is only ever built inside `validate_upgrade`. The fix is a pure relabeling of which side's fields become `local_*` vs `peer_*`:

| field | Initiator's plan | Responder's plan |
|---|---|---|
| `local_identity` | `rq.initiator_identity` | `rq.responder_identity` |
| `peer_identity` | `rq.responder_identity` | `rq.initiator_identity` |
| `local_wireguard_public_key` | `rq.initiator_wireguard_public_key` | `rs.responder_wireguard_public_key` |
| `peer_wireguard_public_key` | `rs.responder_wireguard_public_key` | `rq.initiator_wireguard_public_key` |
| `peer_endpoint` | `rs.responder_wireguard_endpoint` | `rq.initiator_wireguard_endpoint` |
| `local_interface_ips` | `rs.accepted_allowed_ips` | `rq.requested_allowed_ips` |
| `allowed_ips` | `rq.requested_allowed_ips` | `rs.accepted_allowed_ips` |
| `relay_candidates` | `rs.relay_candidates` | `rs.relay_candidates` (shared) |
| `keepalive_seconds` | `rs.keepalive_seconds` | `rs.keepalive_seconds` (shared) |

This holds because `requested_allowed_ips` is already validated (inside `validate_upgrade`) via `overlay.validate_for(view, rq.responder_identity, &rq.requested_allowed_ips)` — i.e. it is the **responder's own** overlay allocation — and `accepted_allowed_ips` is validated via `overlay.validate_for(view, rq.initiator_identity, &rs.accepted_allowed_ips)` — the **initiator's own** allocation. Swapping which one becomes `local_interface_ips` vs `allowed_ips` is exactly the initiator/responder relabeling, nothing more; no new validation logic is needed.

## Verified `PunchPlan` shape (read from `crates/system/nat-traversal/src/punch.rs`, not from memory)

```rust
pub struct PunchPlan {
    pub local_mapped: SocketAddr,
    pub peer_reflexive: SocketAddr,
}
```

`peer_reflexive` is the address a successful hole-punch resolves for the peer — this is what should override a `TunnelInstallPlan`'s statically-advertised `peer_endpoint` before applying it. Important scoping fact: `nat-traversal/src/lib.rs` gates `pub mod punch;` and the `PunchPlan`/`PunchError` re-exports behind `#[cfg(any(test, feature = "simnat"))]` — `PunchPlan` is not part of `nat-traversal`'s production API surface yet (it is exercised only by the in-process `SimNat` rig). `wireguard-effect` therefore must **not** take a hard dependency on `nat-traversal` for this slice — the wiring function accepts a plain `Option<SocketAddr>` override, documented as "pass `punch_plan.peer_reflexive` here once a real caller exists." Wiring an actual `nat-traversal` → `wireguard-effect` caller is out of scope for this slice (it needs `bin/node`/`bin/noded`, which is also out of scope — see "What this plan deliberately does NOT do").

## Global Constraints

- Edition: `edition.workspace = true` (2024), `version.workspace = true` — copy from `crates/system/wireguard-upgrade/Cargo.toml`.
- `wireguard-effect` depends on `wireguard-upgrade` (path dep via the workspace's existing `wireguard-upgrade = { path = "crates/system/wireguard-upgrade" }` entry), never the reverse. It does **not** depend on `nat-traversal` (see above).
- `WireGuardEffect`'s three methods are all `&mut self`, even though `defguard_wireguard_rs`'s own `WireguardInterfaceApi` mixes `&mut self` (`create_interface`) and `&self` (`configure_interface`, `remove_interface`) — uniform `&mut self` keeps the trait simple and lets `FakeWireGuardEffect` record state without interior mutability.
- No `unwrap()`/`expect()` in non-test library code. Test code may use them freely (matches `wireguard-upgrade`'s own style).
- `TunnelInstallPlan` has no public constructor by design (all fields private, only `validate_upgrade`/`validate_upgrade_as` build one) — any test needing a `TunnelInstallPlan` must run a full, real, signed two-party handshake fixture, never attempt to construct the struct directly.
- `validate_upgrade`'s existing signature, behavior, and every existing caller/test in `wireguard-upgrade` must be unchanged (byte-for-byte identical output) after this slice. It becomes a thin wrapper; the real logic moves to `validate_upgrade_as`.
- The `wireguard-upgrade` "relay must be a validator" rule and `relay_candidates` mechanism are untouched — this slice only adds a perspective parameter to plan derivation, nothing about relay eligibility.

---

### Task 1: `wireguard-effect` crate scaffold — `WireGuardEffect` trait + `FakeWireGuardEffect`

**Files:**
- Modify: `Cargo.toml` (workspace root) — add `"crates/system/wireguard-effect"` to `members`, and `wireguard-effect = { path = "crates/system/wireguard-effect" }` to `[workspace.dependencies]`
- Create: `crates/system/wireguard-effect/Cargo.toml`
- Create: `crates/system/wireguard-effect/src/lib.rs`

**Interfaces:**
- Consumes: `defguard_wireguard_rs::InterfaceConfiguration`.
- Produces:
  - `pub trait WireGuardEffect { type Error: std::fmt::Debug; fn create_interface(&mut self) -> Result<(), Self::Error>; fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error>; fn remove_interface(&mut self) -> Result<(), Self::Error>; }`
  - `pub struct FakeWireGuardEffect { pub create_calls: usize, pub remove_calls: usize, pub applied: Vec<InterfaceConfiguration> }` implementing `WireGuardEffect` with `type Error = std::convert::Infallible`.

- [ ] **Step 1: Add the crate to the workspace**

In `Cargo.toml` (root), inside `members = [ ... ]`, add right after the `"crates/system/nat-traversal",` line:

```toml
    "crates/system/wireguard-effect",
```

In the same file's `[workspace.dependencies]` section, add right after the `nat-traversal = { path = "crates/system/nat-traversal" }` line:

```toml
wireguard-effect = { path = "crates/system/wireguard-effect" }
```

- [ ] **Step 2: Write `Cargo.toml` for the crate**

```toml
[package]
name = "wireguard-effect"
edition.workspace = true
version.workspace = true

[dependencies]
defguard_wireguard_rs.workspace = true
wireguard-upgrade = { workspace = true }

[dev-dependencies]
commonware-cryptography = { workspace = true }
```

- [ ] **Step 3: Write the failing test**

Create `crates/system/wireguard-effect/src/lib.rs` with only the doc comment and a test module that references the not-yet-defined `WireGuardEffect`/`FakeWireGuardEffect`:

```rust
//! Effect adapter that takes a validated `wireguard-upgrade` `TunnelInstallPlan`
//! and actually configures a WireGuard interface. `wireguard-upgrade` is a
//! pure validation leaf crate — nothing in the workspace calls
//! `WGApi::configure_interface` today. This crate is that missing consumer:
//! a `WireGuardEffect` trait behind which tests use a deterministic
//! `FakeWireGuardEffect` (CI has no real WireGuard userspace runtime) and
//! real runs use `DefguardWireGuardEffect` (`defguard_wireguard_rs`
//! `WGApi::<Userspace>`).

#[cfg(test)]
mod tests {
    use super::*;
    use defguard_wireguard_rs::{InterfaceConfiguration, key::Key, net::IpAddrMask, peer::Peer};
    use std::net::{IpAddr, Ipv4Addr};

    fn sample_config() -> InterfaceConfiguration {
        let mut peer = Peer::new(Key::new([7u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
            32,
        )]);
        InterfaceConfiguration {
            name: "ducktape-wg0".into(),
            prvkey: "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy".into(),
            addresses: vec![IpAddrMask::new(
                IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
                32,
            )],
            port: 51820,
            peers: vec![peer],
            mtu: None,
            fwmark: None,
        }
    }

    #[test]
    fn fake_records_lifecycle_and_applied_config() {
        let mut fake = FakeWireGuardEffect::default();
        fake.create_interface().unwrap();
        fake.apply(&sample_config()).unwrap();
        fake.remove_interface().unwrap();

        assert_eq!(fake.create_calls, 1);
        assert_eq!(fake.remove_calls, 1);
        assert_eq!(fake.applied.len(), 1);
        assert_eq!(fake.applied[0].name, "ducktape-wg0");
        assert_eq!(fake.applied[0].peers[0].public_key.as_array(), [7u8; 32]);
    }
}
```

- [ ] **Step 4: Run to confirm it fails**

Run: `cargo test -p wireguard-effect`
Expected: FAIL — `FakeWireGuardEffect` (and `WireGuardEffect`) not defined; `cannot find type/trait`.

- [ ] **Step 5: Implement the trait and the fake**

Above the `#[cfg(test)]` block in `crates/system/wireguard-effect/src/lib.rs`, add:

```rust
use defguard_wireguard_rs::InterfaceConfiguration;

/// Effect boundary between a validated WireGuard install plan and the real
/// network stack. `create_interface`/`remove_interface` bracket the
/// interface's lifetime; `apply` pushes a full configuration (private key,
/// listen port, overlay addresses, peer set) to it.
pub trait WireGuardEffect {
    type Error: std::fmt::Debug;

    /// Create the underlying WireGuard interface. Call once before `apply`.
    fn create_interface(&mut self) -> Result<(), Self::Error>;

    /// Apply (create-or-replace) the full interface configuration.
    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error>;

    /// Tear down the interface.
    fn remove_interface(&mut self) -> Result<(), Self::Error>;
}

/// Deterministic in-memory `WireGuardEffect` for tests: records every applied
/// configuration and lifecycle call instead of touching a real network
/// interface. CI has no WireGuard userspace runtime, so this is the only
/// `WireGuardEffect` the automated test suite exercises.
#[derive(Default)]
pub struct FakeWireGuardEffect {
    pub create_calls: usize,
    pub remove_calls: usize,
    pub applied: Vec<InterfaceConfiguration>,
}

impl WireGuardEffect for FakeWireGuardEffect {
    type Error = std::convert::Infallible;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.create_calls += 1;
        Ok(())
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        self.applied.push(config.clone());
        Ok(())
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.remove_calls += 1;
        Ok(())
    }
}
```

Remove the now-redundant `use defguard_wireguard_rs::{InterfaceConfiguration, ...}` duplication inside the test module's `use` line — keep `InterfaceConfiguration` imported there too since the test module needs it directly (it is not brought in by `use super::*` as a *type name* ambiguity issue — both imports refer to the same item, which is allowed). Leave the test module's imports exactly as written in Step 3.

- [ ] **Step 6: Run to confirm it passes**

Run: `cargo test -p wireguard-effect`
Expected: PASS (1 test — `fake_records_lifecycle_and_applied_config`).

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml crates/system/wireguard-effect
git commit -m "feat(wireguard-effect): scaffold crate with WireGuardEffect trait + fake"
```

---

### Task 2: `DefguardWireGuardEffect` — the real userspace adapter

**Files:**
- Create: `crates/system/wireguard-effect/src/defguard_effect.rs`
- Modify: `crates/system/wireguard-effect/src/lib.rs` — wire in the new module

**Interfaces:**
- Consumes: `defguard_wireguard_rs::{WGApi, Userspace, WireguardInterfaceApi, error::WireguardInterfaceError}`, `crate::WireGuardEffect`.
- Produces: `pub struct DefguardWireGuardEffect { .. }` with `pub fn new(ifname: impl Into<String>) -> Result<Self, WireguardInterfaceError>`, implementing `WireGuardEffect` with `type Error = WireguardInterfaceError`. `#[cfg(unix)]`-gated (matches `defguard_wireguard_rs`'s own userspace implementation, which only exists on unix).

- [ ] **Step 1: Write the failing test**

Create `crates/system/wireguard-effect/src/defguard_effect.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructs_without_touching_network() {
        // `WGApi::new` only stores the interface name; it does not open a
        // socket or require privilege. Safe to run in CI.
        let effect = DefguardWireGuardEffect::new("ducktape-wg-test0");
        assert!(effect.is_ok());
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p wireguard-effect defguard_effect::`
Expected: FAIL — module `defguard_effect` does not exist / `DefguardWireGuardEffect` not defined (the module is not yet declared in `lib.rs`, so this also fails to resolve as a path — that is expected at this stage).

- [ ] **Step 3: Declare the module in `lib.rs`**

In `crates/system/wireguard-effect/src/lib.rs`, add near the top (after the crate doc comment, before the `use defguard_wireguard_rs::InterfaceConfiguration;` line):

```rust
#[cfg(unix)]
mod defguard_effect;
#[cfg(unix)]
pub use defguard_effect::DefguardWireGuardEffect;
```

- [ ] **Step 4: Run to confirm it still fails, now on the real symbol**

Run: `cargo test -p wireguard-effect defguard_effect::`
Expected: FAIL — `DefguardWireGuardEffect` not defined in `defguard_effect.rs`.

- [ ] **Step 5: Implement the adapter**

At the top of `crates/system/wireguard-effect/src/defguard_effect.rs` (above the `#[cfg(test)]` block), add:

```rust
use defguard_wireguard_rs::{
    InterfaceConfiguration, Userspace, WGApi, WireguardInterfaceApi, error::WireguardInterfaceError,
};

use crate::WireGuardEffect;

/// Real `WireGuardEffect` backed by `defguard_wireguard_rs`'s userspace
/// (BoringTun) implementation. Not exercised by the automated test suite —
/// CI has no WireGuard userspace runtime, and `create_interface`/`apply`
/// require a privileged host (root or `CAP_NET_ADMIN`) with BoringTun
/// reachable at `/var/run/wireguard/<ifname>.sock`. Verify this path
/// manually, cross-machine, using the `real_userspace_lifecycle_smoke`
/// `#[ignore]`d test below: `cargo test -p wireguard-effect --
/// --ignored real_userspace_lifecycle_smoke` on a Linux box with root, then
/// confirm with `ip addr show <ifname>` and `wg show <ifname>`.
pub struct DefguardWireGuardEffect {
    api: WGApi<Userspace>,
}

impl DefguardWireGuardEffect {
    /// Construct the wrapper for the named interface. This only allocates
    /// the `WGApi` handle — it does not touch the network or require
    /// privilege; `create_interface` does.
    pub fn new(ifname: impl Into<String>) -> Result<Self, WireguardInterfaceError> {
        Ok(Self {
            api: WGApi::<Userspace>::new(ifname)?,
        })
    }
}

impl WireGuardEffect for DefguardWireGuardEffect {
    type Error = WireguardInterfaceError;

    fn create_interface(&mut self) -> Result<(), Self::Error> {
        self.api.create_interface()
    }

    fn apply(&mut self, config: &InterfaceConfiguration) -> Result<(), Self::Error> {
        self.api.configure_interface(config)
    }

    fn remove_interface(&mut self) -> Result<(), Self::Error> {
        self.api.remove_interface()
    }
}
```

- [ ] **Step 6: Run to confirm the CI-safe test passes**

Run: `cargo test -p wireguard-effect defguard_effect::`
Expected: PASS (1 test — `constructs_without_touching_network`).

- [ ] **Step 7: Add the documented, `#[ignore]`d real-lifecycle recipe**

In the same `#[cfg(test)] mod tests` block in `defguard_effect.rs`, add a second test:

```rust
    #[test]
    #[ignore = "requires root + a running WireGuard userspace (BoringTun) runtime; run manually, cross-machine: cargo test -p wireguard-effect -- --ignored real_userspace_lifecycle_smoke"]
    fn real_userspace_lifecycle_smoke() {
        use defguard_wireguard_rs::{key::Key, net::IpAddrMask, peer::Peer};
        use std::net::{IpAddr, Ipv4Addr};

        let mut effect = DefguardWireGuardEffect::new("ducktape-wg-smoke0").unwrap();
        effect.create_interface().unwrap();

        let mut peer = Peer::new(Key::new([9u8; 32]));
        peer.set_allowed_ips(vec![IpAddrMask::new(
            IpAddr::V4(Ipv4Addr::new(100, 64, 0, 2)),
            32,
        )]);
        let config = InterfaceConfiguration {
            name: "ducktape-wg-smoke0".into(),
            prvkey: "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy".into(),
            addresses: vec![IpAddrMask::new(
                IpAddr::V4(Ipv4Addr::new(100, 64, 0, 1)),
                32,
            )],
            port: 51820,
            peers: vec![peer],
            mtu: None,
            fwmark: None,
        };
        effect.apply(&config).unwrap();
        effect.remove_interface().unwrap();
    }
```

- [ ] **Step 8: Run the full crate suite (ignored test stays skipped by default)**

Run: `cargo test -p wireguard-effect`
Expected: PASS (2 tests run: `fake_records_lifecycle_and_applied_config`, `constructs_without_touching_network`; 1 ignored: `real_userspace_lifecycle_smoke`).

- [ ] **Step 9: Commit**

```bash
git add crates/system/wireguard-effect/src/defguard_effect.rs crates/system/wireguard-effect/src/lib.rs
git commit -m "feat(wireguard-effect): DefguardWireGuardEffect real userspace adapter"
```

---

### Task 3: Resolve the responder-perspective plan gap in `wireguard-upgrade`

**Files:**
- Modify: `crates/system/wireguard-upgrade/src/lib.rs`
- Modify: `crates/system/wireguard-upgrade/tests/tunnel_e2e.rs`

**Interfaces:**
- Produces: `pub enum Perspective { Initiator, Responder }` and `pub fn validate_upgrade_as(perspective: Perspective, view: &MeshView, policy: &PortPolicy, overlay: &OverlayPolicy, current_view: u64, request: &TunnelUpgradeRequest, response: &TunnelUpgradeResponse, ack: &TunnelUpgradeAck, replay: &mut ReplayCache) -> Result<TunnelInstallPlan, UpgradeError>`. `validate_upgrade` becomes `validate_upgrade_as(Perspective::Initiator, ..)` — identical signature, identical output for every existing caller.

- [ ] **Step 1: Write the failing test**

In `crates/system/wireguard-upgrade/tests/tunnel_e2e.rs`, insert a new test directly after `both_parties_validate_but_the_plan_is_initiator_local` (before the `// ── the doc-mandated fixed vector ──` section comment):

```rust
/// resolves the gap pinned above: `validate_upgrade_as` lets the RESPONDER
/// derive its own install plan from the identical signed triple, without
/// weakening or duplicating the validation `validate_upgrade` already does
/// (the same checks run once, from the responder's own view + replay
/// cache, with the responder's own perspective).
#[test]
fn responder_derives_its_own_install_plan_from_validate_upgrade_as() {
    let (a, b, c, set) = three_party_epoch(7, 9, 8);
    let policy = PortPolicy::production();
    let overlay = OverlayPolicy::default_v4();
    let ads = advertisements(
        &[
            (&a, 10, vec![]),
            (&b, 20, vec![]),
            (&c, 30, vec![MeshCapability::Relay]),
        ],
        &set,
    );
    let view_a = MeshView::verify(set.clone(), ads.clone(), &policy, 10).unwrap();
    let mut reversed = ads.clone();
    reversed.reverse();
    let view_b = MeshView::verify(set.clone(), reversed, &policy, 10).unwrap();

    let hs = handshake(
        &a,
        &b,
        xkey(0x0a),
        xkey(0x0b),
        &view_a,
        &policy,
        &overlay,
        None,
    );

    // the initiator's plan, exactly as the pinned test already covers.
    let mut cache_a = ReplayCache::default();
    let plan_a = validate_upgrade(
        &view_a,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_a,
    )
    .unwrap();

    // the responder validates the SAME triple against its OWN view and its
    // OWN replay cache, asking for ITS perspective in one call.
    let mut cache_b = ReplayCache::default();
    let plan_b = validate_upgrade_as(
        Perspective::Responder,
        &view_b,
        &policy,
        &overlay,
        12,
        &hs.request,
        &hs.response,
        &hs.ack,
        &mut cache_b,
    )
    .unwrap();

    // b's plan is local-to-b: the mirror image of a's plan, not a copy of it.
    assert_eq!(plan_b.local_identity(), id(&b));
    assert_eq!(plan_b.peer_identity(), id(&a));
    assert_eq!(plan_b.local_wireguard_public_key(), xkey(0x0b));
    assert_eq!(plan_b.peer_wireguard_public_key(), xkey(0x0a));
    assert_eq!(
        plan_b.peer_endpoint(),
        view_b.record(id(&a)).unwrap().wireguard_endpoint
    );
    assert_eq!(
        plan_b.local_interface_ips(),
        overlay.allowed_ips_for(&view_b, id(&b)).unwrap().as_slice()
    );
    assert_eq!(
        plan_b.allowed_ips(),
        overlay.allowed_ips_for(&view_b, id(&a)).unwrap().as_slice()
    );
    assert_ne!(plan_a, plan_b);

    // complementary: a's local address is what b routes to, and vice versa.
    assert_eq!(plan_a.local_interface_ips(), plan_b.allowed_ips());
    assert_eq!(plan_b.local_interface_ips(), plan_a.allowed_ips());
    assert_eq!(
        plan_a.peer_wireguard_public_key(),
        plan_b.local_wireguard_public_key()
    );
    assert_eq!(
        plan_b.peer_wireguard_public_key(),
        plan_a.local_wireguard_public_key()
    );

    // b's plan converts into its OWN concrete defguard peer + interface
    // configuration, targeting a — proving both parties, not just the
    // initiator, can now bring up their side of the tunnel.
    let peer_cfg = DefguardPeerConfig::from_plan(&plan_b);
    assert_eq!(
        peer_cfg.peer.endpoint,
        Some(
            view_b
                .record(id(&a))
                .unwrap()
                .wireguard_endpoint
                .socket_addr()
        )
    );
    assert_eq!(peer_cfg.peer.persistent_keepalive_interval, Some(25));
    assert_eq!(peer_cfg.allowed_ips, plan_b.allowed_ips());

    let listen_b = view_b.record(id(&b)).unwrap().wireguard_endpoint;
    let iface_b = DefguardInterfaceConfig::from_plan(
        "ducktape-wg0",
        "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
        listen_b,
        vec![plan_b.clone()],
    );
    assert_eq!(iface_b.config.port, 51820);
    assert_eq!(iface_b.config.peers.len(), 1);
    assert_eq!(
        iface_b.config.addresses.len(),
        plan_b.local_interface_ips().len()
    );
}
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p wireguard-upgrade --test tunnel_e2e responder_derives_its_own_install_plan_from_validate_upgrade_as`
Expected: FAIL — compile error, `Perspective`/`validate_upgrade_as` not found in `wireguard_upgrade`.

- [ ] **Step 3: Add `Perspective` and refactor `validate_upgrade` into `validate_upgrade_as`**

In `crates/system/wireguard-upgrade/src/lib.rs`, replace the entire existing `validate_upgrade` function (currently lines 823–964, from `#[allow(clippy::too_many_arguments)]\npub fn validate_upgrade(` through its closing `}`) with:

```rust
/// Which side of a validated handshake a [`TunnelInstallPlan`] is built for.
/// [`validate_upgrade`] (unchanged, kept for existing callers) always builds
/// the initiator's plan; [`validate_upgrade_as`] lets either party derive its
/// OWN install plan from the identical signed triple.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Perspective {
    Initiator,
    Responder,
}

#[allow(clippy::too_many_arguments)]
pub fn validate_upgrade(
    view: &MeshView,
    policy: &PortPolicy,
    overlay: &OverlayPolicy,
    current_view: u64,
    request: &TunnelUpgradeRequest,
    response: &TunnelUpgradeResponse,
    ack: &TunnelUpgradeAck,
    replay: &mut ReplayCache,
) -> Result<TunnelInstallPlan, UpgradeError> {
    validate_upgrade_as(
        Perspective::Initiator,
        view,
        policy,
        overlay,
        current_view,
        request,
        response,
        ack,
        replay,
    )
}

/// Identical validation to [`validate_upgrade`], but returns the install plan
/// for the requested `perspective`. The initiator and responder each hold a
/// full copy of the same signed request/response/ack triple; each calls this
/// ONCE, from its own `MeshView` and its own `ReplayCache`, with its own
/// perspective, to derive its own `local_*`/`peer_*` install config. This is
/// the responder-side counterpart the `tunnel_e2e` "PINNED GAP" test
/// documents: before this function existed, only the initiator's plan was
/// derivable.
#[allow(clippy::too_many_arguments)]
pub fn validate_upgrade_as(
    perspective: Perspective,
    view: &MeshView,
    policy: &PortPolicy,
    overlay: &OverlayPolicy,
    current_view: u64,
    request: &TunnelUpgradeRequest,
    response: &TunnelUpgradeResponse,
    ack: &TunnelUpgradeAck,
    replay: &mut ReplayCache,
) -> Result<TunnelInstallPlan, UpgradeError> {
    validate_admission_root(view.active_set.admission_root)?;
    request.verify_signature()?;
    response.verify_signature()?;
    ack.verify_signature()?;

    let rq = &request.fields;
    let rs = &response.fields;
    let ak = &ack.fields;
    let root = view.active_set.valset_root;
    let admission_root = view.active_set.admission_root;
    if rq.request_tuple()
        != (
            view.active_set.namespace.as_str(),
            view.active_set.epoch,
            root,
            admission_root,
            view.mesh_version,
        )
        || rs.request_hash != request.hash()
        || rs.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                admission_root,
                view.mesh_version,
            )
        || ak.request_hash != request.hash()
        || ak.response_hash != response.hash()
        || ak.request_tuple()
            != (
                view.active_set.namespace.as_str(),
                view.active_set.epoch,
                root,
                admission_root,
                view.mesh_version,
            )
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    if rq.port_policy_hash != policy.hash() {
        return Err(UpgradeError::PortPolicyMismatch);
    }
    if rq.initiator_identity != rs.initiator_identity
        || rq.initiator_identity != ak.initiator_identity
        || rq.responder_identity != rs.responder_identity
        || rq.responder_identity != ak.responder_identity
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    let initiator_record = view
        .record(rq.initiator_identity)
        .ok_or(UpgradeError::UnknownValidator)?;
    let responder_record = view
        .record(rq.responder_identity)
        .ok_or(UpgradeError::UnknownValidator)?;
    if rq.initiator_wireguard_endpoint != initiator_record.wireguard_endpoint
        || rs.responder_wireguard_endpoint != responder_record.wireguard_endpoint
    {
        return Err(UpgradeError::HandshakeMismatch);
    }
    ensure_x25519(rq.initiator_wireguard_public_key)?;
    ensure_x25519(rs.responder_wireguard_public_key)?;
    policy.check_endpoint(&rq.initiator_wireguard_endpoint)?;
    policy.check_endpoint(&rs.responder_wireguard_endpoint)?;
    if current_view > rq.expires_at_view
        || current_view > rs.expires_at_view
        || current_view > ak.expires_at_view
    {
        return Err(UpgradeError::Expired);
    }
    if ak.installed_at_view > current_view
        || current_view.saturating_sub(ak.installed_at_view) > MAX_ACK_INSTALL_LAG
    {
        return Err(UpgradeError::BadAckView);
    }
    overlay.validate_for(view, rq.responder_identity, &rq.requested_allowed_ips)?;
    overlay.validate_for(view, rq.initiator_identity, &rs.accepted_allowed_ips)?;
    if !rs.relay_candidates.is_empty() && rs.direct_dial_failure.is_none() {
        return Err(UpgradeError::InvalidRelay);
    }
    if let Some(failure) = &rs.direct_dial_failure {
        validate_direct_dial_failure(
            failure,
            view,
            current_view,
            rq.initiator_identity,
            rq.responder_identity,
            responder_record.wireguard_endpoint,
        )?;
    }
    for relay in &rs.relay_candidates {
        let record = view.record(*relay).ok_or(UpgradeError::InvalidRelay)?;
        if !record.capabilities.contains(&MeshCapability::Relay) {
            return Err(UpgradeError::InvalidRelay);
        }
    }
    let mut replay_keys = vec![
        (rq.initiator_identity, rq.epoch, rq.nonce),
        (rs.responder_identity, rs.epoch, rs.nonce),
        (ak.initiator_identity, ak.epoch, ak.nonce),
    ];
    if let Some(failure) = &rs.direct_dial_failure {
        let f = &failure.fields;
        replay_keys.push((f.observer_identity, f.epoch, f.nonce));
    }
    let replay_keys = replay.check_batch(&replay_keys)?;

    for (identity, epoch, nonce) in replay_keys {
        replay.insert(identity, epoch, nonce);
    }

    let context = TunnelInstallContext {
        namespace: view.active_set.namespace.clone(),
        epoch: view.active_set.epoch,
        valset_root: root,
        admission_root,
        mesh_version: view.mesh_version,
    };
    Ok(match perspective {
        Perspective::Initiator => TunnelInstallPlan {
            context,
            local_identity: rq.initiator_identity,
            peer_identity: rq.responder_identity,
            local_wireguard_public_key: rq.initiator_wireguard_public_key,
            peer_wireguard_public_key: rs.responder_wireguard_public_key,
            peer_endpoint: rs.responder_wireguard_endpoint,
            local_interface_ips: rs.accepted_allowed_ips.clone(),
            allowed_ips: rq.requested_allowed_ips.clone(),
            relay_candidates: rs.relay_candidates.clone(),
            keepalive_seconds: rs.keepalive_seconds,
        },
        Perspective::Responder => TunnelInstallPlan {
            context,
            local_identity: rq.responder_identity,
            peer_identity: rq.initiator_identity,
            local_wireguard_public_key: rs.responder_wireguard_public_key,
            peer_wireguard_public_key: rq.initiator_wireguard_public_key,
            peer_endpoint: rq.initiator_wireguard_endpoint,
            local_interface_ips: rq.requested_allowed_ips.clone(),
            allowed_ips: rs.accepted_allowed_ips.clone(),
            relay_candidates: rs.relay_candidates.clone(),
            keepalive_seconds: rs.keepalive_seconds,
        },
    })
}
```

- [ ] **Step 4: Run to confirm the new test passes and nothing regresses**

Run: `cargo test -p wireguard-upgrade`
Expected: PASS — every prior test (`both_parties_validate_but_the_plan_is_initiator_local`, `mesh_version_v1_fixed_vector`, `epoch_cutover_revokes_departed_validators_and_rekeys_survivors`, `relay_fallback_uses_only_admitted_relay_validators`, `upgrade_protocol.rs`'s tests, all inline unit tests) plus the new `responder_derives_its_own_install_plan_from_validate_upgrade_as`.

- [ ] **Step 5: Update the now-stale doc comments (no assertion changes — the gap is resolved, the pin stays historically accurate)**

In `crates/system/wireguard-upgrade/tests/tunnel_e2e.rs`, replace the module doc comment's closing paragraph (currently ending `//! wiring gap is load-bearing and pinned here: ... (see \`both_parties_validate_but_the_plan_is_initiator_local\`).`) with:

```rust
//! this drives the protocol crate exactly as far as the product reaches today:
//! `wireguard-upgrade` is a LEAF crate (no consumer in bin/node or bin/noded),
//! so the effectful boundary — actually applying a `DefguardInterfaceConfig`
//! through `WGApi` — is out of e2e reach until the node wiring lands (that
//! wiring now lives in `crates/system/wireguard-effect`, Slice 0b). one gap
//! WAS load-bearing and pinned here: `validate_upgrade` only emits the
//! INITIATOR-perspective plan (`local = initiator`), so a responder could
//! fully validate the handshake but could not derive ITS install config from
//! the returned plan (see `both_parties_validate_but_the_plan_is_initiator_local`).
//! Resolved by `validate_upgrade_as(Perspective::Responder, ..)` — see
//! `responder_derives_its_own_install_plan_from_validate_upgrade_as` below.
```

And replace the pinned test's trailing comment (the paragraph starting `// the plan is a deterministic function of the triple — but it is ALWAYS` through `// in the node wiring, not something these asserts can guard.`) with:

```rust
    // `validate_upgrade` is ALWAYS the initiator's perspective — calling it
    // from b's own view still returns a's plan (`plan_b.local_identity() ==
    // a`), because `validate_upgrade` hardcodes `Perspective::Initiator`
    // (kept exactly as-is so no existing caller's behavior changes). This is
    // no longer a gap: `validate_upgrade_as(Perspective::Responder, ..)` lets
    // b derive ITS OWN install plan from the identical triple — see
    // `responder_derives_its_own_install_plan_from_validate_upgrade_as`
    // below.
```

Leave every assertion in `both_parties_validate_but_the_plan_is_initiator_local` untouched.

- [ ] **Step 6: Run the full crate suite once more**

Run: `cargo test -p wireguard-upgrade`
Expected: PASS (doc-comment-only change; same test count and results as Step 4).

- [ ] **Step 7: Commit**

```bash
git add crates/system/wireguard-upgrade/src/lib.rs crates/system/wireguard-upgrade/tests/tunnel_e2e.rs
git commit -m "feat(wireguard-upgrade): validate_upgrade_as resolves the responder-plan gap"
```

---

### Task 4: `apply_tunnel_plan` — wiring a validated plan + peer-endpoint override through a `WireGuardEffect`

**Files:**
- Create: `crates/system/wireguard-effect/src/wiring.rs`
- Modify: `crates/system/wireguard-effect/src/lib.rs` — wire in the new module

**Interfaces:**
- Consumes: `wireguard_upgrade::{DefguardInterfaceConfig, Endpoint, TunnelInstallPlan}`, `crate::WireGuardEffect`.
- Produces: `pub fn apply_tunnel_plan<E: WireGuardEffect>(effect: &mut E, ifname: impl Into<String>, private_key_base64: impl Into<String>, listen_endpoint: Endpoint, plan: &TunnelInstallPlan, peer_endpoint_override: Option<SocketAddr>) -> Result<(), E::Error>`.

- [ ] **Step 1: Write the failing test**

Create `crates/system/wireguard-effect/src/wiring.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use commonware_cryptography::{Signer as _, ed25519::PrivateKey};
    use defguard_wireguard_rs::net::IpAddrMask;
    use std::net::{IpAddr, Ipv4Addr};
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

    /// a minimal two-validator handshake, direct (no relay), yielding the
    /// INITIATOR's (a's) validated install plan and a's own listen endpoint —
    /// everything `apply_tunnel_plan` needs. `TunnelInstallPlan` has no
    /// public constructor by design (only `validate_upgrade`/
    /// `validate_upgrade_as` produce one), so this fixture runs the real
    /// signed handshake exactly like `wireguard-upgrade`'s own e2e tests do.
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
    fn applies_plan_with_punch_resolved_peer_endpoint() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect::default();
        let override_addr: SocketAddr = "203.0.113.9:51820".parse().unwrap();

        apply_tunnel_plan(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &plan,
            Some(override_addr),
        )
        .unwrap();

        assert_eq!(fake.create_calls, 1);
        assert_eq!(fake.applied.len(), 1);
        let applied = &fake.applied[0];
        assert_eq!(applied.peers.len(), 1);
        let peer = &applied.peers[0];
        assert_eq!(peer.public_key.as_array(), plan.peer_wireguard_public_key().0);
        assert_eq!(peer.endpoint, Some(override_addr));
        assert_eq!(
            peer.allowed_ips,
            plan.allowed_ips()
                .iter()
                .map(|r| IpAddrMask::new(r.addr, r.cidr))
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn falls_back_to_the_plans_own_endpoint_when_no_override_is_given() {
        let (plan, listen) = two_party_plan();
        let mut fake = crate::FakeWireGuardEffect::default();

        apply_tunnel_plan(
            &mut fake,
            "ducktape-wg0",
            "cHJpdmF0ZS1rZXktYmFzZTY0LXBsYWNlaG9sZGVy",
            listen,
            &plan,
            None,
        )
        .unwrap();

        assert_eq!(
            fake.applied[0].peers[0].endpoint,
            Some(plan.peer_endpoint().socket_addr())
        );
    }
}
```

- [ ] **Step 2: Run to confirm it fails**

Run: `cargo test -p wireguard-effect wiring::`
Expected: FAIL — `apply_tunnel_plan` not defined, and the `wiring` module is not yet declared in `lib.rs`.

- [ ] **Step 3: Declare the module in `lib.rs`**

In `crates/system/wireguard-effect/src/lib.rs`, add alongside the existing `#[cfg(unix)] mod defguard_effect;` block:

```rust
mod wiring;
pub use wiring::apply_tunnel_plan;
```

- [ ] **Step 4: Run to confirm it still fails on the real symbol**

Run: `cargo test -p wireguard-effect wiring::`
Expected: FAIL — `apply_tunnel_plan` not defined in `wiring.rs`.

- [ ] **Step 5: Implement the wiring function**

At the top of `crates/system/wireguard-effect/src/wiring.rs` (above the `#[cfg(test)]` block), add:

```rust
use std::net::SocketAddr;

use wireguard_upgrade::{DefguardInterfaceConfig, Endpoint, TunnelInstallPlan};

use crate::WireGuardEffect;

/// Apply a validated `TunnelInstallPlan` through a `WireGuardEffect`,
/// bringing up (or replacing) the local WireGuard interface for this one
/// peer relationship.
///
/// `peer_endpoint_override`, when set, replaces the plan's statically
/// advertised `peer_endpoint` with a different address before applying —
/// this is how a punched or relayed path gets wired in without touching
/// `wireguard-upgrade`'s validated plan: the caller passes the hole-punch's
/// resolved reflexive address
/// (`nat_traversal::punch::PunchPlan::peer_reflexive` in the simulated rig;
/// a real `NatClient` observation in production) or, on hole-punch failure,
/// a coordinator relay socket. `None` uses the plan's own advertised
/// endpoint unchanged (the direct, no-NAT-surprises case).
pub fn apply_tunnel_plan<E: WireGuardEffect>(
    effect: &mut E,
    ifname: impl Into<String>,
    private_key_base64: impl Into<String>,
    listen_endpoint: Endpoint,
    plan: &TunnelInstallPlan,
    peer_endpoint_override: Option<SocketAddr>,
) -> Result<(), E::Error> {
    let mut iface = DefguardInterfaceConfig::from_plan(
        ifname,
        private_key_base64,
        listen_endpoint,
        vec![plan.clone()],
    );
    if let Some(addr) = peer_endpoint_override {
        for peer in &mut iface.config.peers {
            peer.endpoint = Some(addr);
        }
    }
    effect.create_interface()?;
    effect.apply(&iface.config)
}
```

- [ ] **Step 6: Run to confirm it passes**

Run: `cargo test -p wireguard-effect wiring::`
Expected: PASS (2 tests — `applies_plan_with_punch_resolved_peer_endpoint`, `falls_back_to_the_plans_own_endpoint_when_no_override_is_given`).

- [ ] **Step 7: Run the whole crate**

Run: `cargo test -p wireguard-effect`
Expected: PASS (4 tests run, 1 ignored — the two from Task 1/2 plus the two new ones).

- [ ] **Step 8: Commit**

```bash
git add crates/system/wireguard-effect/src/wiring.rs crates/system/wireguard-effect/src/lib.rs
git commit -m "feat(wireguard-effect): apply_tunnel_plan wires a plan + endpoint override through a WireGuardEffect"
```

---

### Task 5: Full slice gate

**Files:** none (verification only).

- [ ] **Step 1: Run the exact merge gate for this slice**

Run: `cargo test -p wireguard-upgrade -p wireguard-effect --all-features && cargo clippy -p wireguard-upgrade -p wireguard-effect --all-features -- -D warnings`
Expected: PASS — every test in both crates green (including the Task 3 addition and the four in `wireguard-effect`, `real_userspace_lifecycle_smoke` reported as ignored, not failed), zero clippy warnings.

- [ ] **Step 2: Confirm the whole workspace still builds** (new crate wiring didn't break an unrelated crate's resolution)

Run: `cargo check --workspace`
Expected: PASS.

- [ ] **Step 3: If `Cargo.lock` changed (new crate entry), stage it**

Run: `git status --short Cargo.lock`
Expected: either no output (already committed by an earlier task's `cargo test` run) or a modified/untracked `Cargo.lock` — if the latter:

```bash
git add Cargo.lock
git commit -m "chore: update Cargo.lock for wireguard-effect"
```

If `Cargo.lock` was already clean, skip this commit — there is nothing to do.

---

## What this plan deliberately does NOT do (deferred to later slices/plans)

- **No `Kernel` variant adapter.** Only `WGApi::<Userspace>` (BoringTun) is wrapped; a `WGApi::<Kernel>` adapter for native kernel WireGuard is a follow-on if userspace throughput proves insufficient.
- **No per-peer add/remove API.** `WireGuardEffect::apply` always pushes a full `InterfaceConfiguration` (matching `configure_interface`'s own all-at-once semantics); a narrower `configure_peer`/`remove_peer` surface is deferred until a caller needs incremental peer churn without a full re-apply.
- **No `bin/node`/`bin/noded` consumer.** This slice makes the effect *wireable*; actually calling `apply_tunnel_plan` from the running node (deciding *when* to bring up a tunnel, holding the `DefguardWireGuardEffect` for the process lifetime, retrying on `WireguardInterfaceError`) is out of scope here and belongs to a slice that also wires `nat-traversal`'s real (non-`SimNat`) discovery/punch path in.
- **No `nat-traversal` integration.** `wireguard-effect` takes a plain `Option<SocketAddr>` override, not a `PunchPlan` — `nat-traversal::punch::PunchPlan` is itself gated `#[cfg(any(test, feature = "simnat"))]` (simulation-only today). Wiring `PunchPlan.peer_reflexive` (or a real `NatClient` equivalent) into this override is later-slice work.
- **No coordinator relay fallback selection.** The `peer_endpoint_override` mechanism is generic enough to carry a relay socket later (Slice 2's ciphertext relay), but choosing *when* to use a relay vs. a punched address is not implemented here.
- **No cross-machine acceptance run.** The `real_userspace_lifecycle_smoke` `#[ignore]`d test documents the manual recipe; actually executing it on two real machines is part of the epic's Acceptance item 2, tracked separately.

## Self-review notes

- **Spec coverage:** implements spec §"Component 5 WireGuard effect wiring" in full (the `WireGuardEffect` trait, fake-for-tests/defguard-for-real split, and the plan→apply wiring with an endpoint override for the punched/relayed case) and directly resolves the `tunnel_e2e.rs` PINNED GAP referenced by §"Component 6" ("a responder cannot derive ITS install config"). §"Component 6"'s relay-concept separation itself is validated as already-intact and untouched — no code in this slice touches `relay_candidates` eligibility or the "relay must be a validator" rule.
- **Type/API consistency:** every type named in a later task (`WireGuardEffect`, `FakeWireGuardEffect`, `DefguardWireGuardEffect`, `Perspective`, `validate_upgrade_as`, `apply_tunnel_plan`) is defined with the identical signature everywhere it's used, including in tests. `TunnelInstallPlan`'s private-field, validated-construction-only invariant is respected throughout — no test anywhere constructs one directly.
- **No placeholders:** every step carries real, complete code (no `todo!()`, no elided function bodies) and an exact `cargo` command with its expected result. The `defguard_wireguard_rs` 0.10 API surface used here (`WGApi::new`, `WireguardInterfaceApi::{create_interface,configure_interface,remove_interface}`, `InterfaceConfiguration`, `Peer`, `IpAddrMask`, `Key`, `WireguardInterfaceError`) was read directly from the vendored crate source, not recalled from training data.
- **Backward compatibility verified:** `validate_upgrade`'s body in Task 3 is a byte-for-byte copy of the original logic (only extracted into `validate_upgrade_as` and invoked with `Perspective::Initiator`), so every existing test in `tunnel_e2e.rs` and `upgrade_protocol.rs` keeps passing unmodified — confirmed by Step 4/6's "run full suite" checks in Task 3.
