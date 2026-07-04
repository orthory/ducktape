# Private-Cutover Integration Gap — the node-wiring handoff

The precise engineering handoff for closing the private-cutover epic: exactly
what remains to wire `nat-traversal` (`NatClient`) and `wireguard-effect` into
`ducktape-node`'s networking loop, the composition decision against commonware
`authenticated::discovery`, the coordinator-side relay-bind fix, and why the real
cross-machine run needs the user's infra. It is written so a future implementer
(the "Slice 5 — node reachability wiring" follow-on) can pick the work up cold.

Design of record:
[Private Cutover — Coordinator](superpowers/specs/2026-07-05-private-cutover-coordinator-design.md).

## §1 — Current state (what's proven, what's unwired)

The **reachability mechanism is fully built and CI-proven** as library crates:

- `crates/system/nat-traversal` — STUN reflexive discovery, rendezvous,
  UDP hole-punch (simultaneous open), and a ciphertext relay. The
  simulated-NAT suite `crates/system/nat-traversal/tests/simnat_ci.rs`
  (Slice 3) deterministically covers reflexive discovery, hole-punch success,
  hole-punch failure → relay splice, endpoint churn re-advertisement, and the
  v3 invite verify/reject paths.
- `crates/system/wireguard-effect` — `trait WireGuardEffect`
  (`create_interface` / `apply` / `remove_interface`), `apply_tunnel_plan(...)`,
  a real `DefguardWireGuardEffect` (unix userspace `WGApi`), and a
  `FakeWireGuardEffect` for tests. Its module doc states plainly: **nothing in
  the workspace calls `WGApi::configure_interface` today.**
- **Slice 1** wired v3 invite *encoding* into `bin/node/src/config.rs`
  (`INVITE_PREFIX_V3`, `enum Reach { Direct, Fronted, Coordinated }`, `CoordRef`,
  `ReachHint`), with v2 kept parse-only.

**But the live node calls none of it.** Verified at HEAD:

```
grep -rn 'nat-traversal|wireguard-effect|nat_traversal|wireguard_effect' bin/node bin/noded
  → NONE FOUND
```

Neither reachability crate is a dependency of `bin/node`/`bin/noded`. The mesh is
built solely from commonware `authenticated::discovery`
(`bin/node/src/main.rs` — `discovery::Config::local(...)` ~2641,
`Network::new(...)` ~2649). Concretely, the live `ducktape-node` lacks **four
capabilities**:

1. **Reflexive discovery** — it never constructs a `NatClient`, never sends a
   `BindRequest`, never learns its NAT-mapped `ip:port`.
2. **Hole-punch** — it never drives `send_punch_to`/`recv_punch_from`.
3. **WireGuard bring-up** — it never calls `apply_tunnel_plan` /
   `DefguardWireGuardEffect`; no interface is ever created.
4. **Relay** — it never calls `request_relay` and never injects a relay socket as
   a peer endpoint override.

## §2 — The composition decision (reachability plane vs. `authenticated::discovery`)

The load-bearing design call, argued explicitly.

**There are two planes.**

- The **control mesh** is commonware `authenticated::discovery` over **TCP**
  (`main.rs` ~2641). It is already key-authenticated end-to-end and already
  frontable via the Phase-1 sentry / coordinator-fronting entry design
  (`docs/sentry-deployment.md`). It stays TCP; it is **not** the thing this epic
  hole-punches.
- The **data tunnel** is validator↔validator **WireGuard** (the
  `wireguard-upgrade` protocol). *That* is what needs reflexive discovery +
  hole-punch + effect bring-up + relay.

**Decision: the reachability plane composes _orthogonally_ to
`authenticated::discovery`.** The node runs a `NatClient` on its **own UDP
socket** to (a) discover its reflexive and publish it into the signed
`EndpointRecord.wireguard_endpoint` advertisement (wrapped by
`EndpointAdvertisement`, verified through `MeshView::verify`), and (b)
rendezvous/punch for the **WireGuard** endpoint. It then drives
`wireguard_effect::apply_tunnel_plan(..., peer_endpoint_override =
punched_reflexive_or_relay)` on tunnel upgrade. commonware's TCP dialer is
**untouched**.

**Why this is right:** the mesh is already key-authenticated end-to-end (so any
network path is safe) and already frontable, so there is no reason to fold
STUN/punch into the TCP transport. Doing so would duplicate the entry plane and
entangle two independently-shippable layers (the TCP control plane and the
WireGuard data plane) that today ship and are tested separately.

**Corollary (verified stub to fix).** `Reach::Coordinated` hints must be **split
out** of `reach_entries()` / the TCP `bootstrappers` list
(`bin/node/src/config.rs` ~337-354, where `Reach::Coordinated(c) => &c.coord_addr`
is pushed straight into the `(expected_key, coord_addr)` list that becomes the
commonware TCP bootstrappers at `main.rs` ~2540/2646) and handed to the
`NatClient` instead. Today a `Coordinated` hint is dialed as an ordinary TCP mesh
peer at the coordinator's **UDP** address — a no-op-at-best. The alternative
(dial the coordinator over TCP as a mesh forwarder) is explicitly **out** per the
design's non-goals and the two-relay separation.

## §3 — The specific integration points (file-anchored checklist)

1. **Add deps.** `bin/node/Cargo.toml` gains `nat-traversal` + `wireguard-effect`
   (workspace). Decide: inline in `bin/node`, or a thin new
   `crates/system/reachability` orchestrator that `bin/node` owns the config
   plumbing for. **Recommend the orchestrator crate** — it keeps `bin/node` lean
   and is unit-testable against `FakeWireGuardEffect` + `SimNat` — but note
   `bin/node` already owns the `Reach`/`CoordRef` types, so the split has a seam
   to design (the orchestrator takes resolved `(coord_addr, coord_key)` pairs;
   `bin/node` keeps invite parsing).
2. **Route `Coordinated` hints.** `bin/node/src/config.rs` — add a
   `coordinator_refs() -> Vec<(coord_addr, coord_key)>` beside `reach_entries()`
   and **stop pushing `Coordinated` into the TCP bootstrapper list** (~343). The
   two accessors partition the reach hints: `Direct`/`Fronted` → TCP
   bootstrappers, `Coordinated` → the `NatClient`.
3. **Boot the client.** At mesh boot (`bin/node/src/main.rs` ~2641, alongside
   `Network::new` / `network.start()`), construct
   `NatClient::bind_multi(node_key, coord_addrs)`;
   `discover_reflexive_failover(per_try)`; publish the reflexive into
   `EndpointRecord.wireguard_endpoint` with a monotonic nonce; `readvertise(nonce+1)`
   on rebind (mirrors the dup rule that `MeshView::verify` enforces on
   re-advertisement, which Slice 3's `nat_traversal::AdvertBook`
   (`crates/system/nat-traversal/src/advert.rs`) already models).
4. **Rendezvous + punch.** `register()`, then per data-peer `lookup(peer)` /
   `recv_punch_sync()` to learn the peer's reflexive, and
   `send_punch_to`/`recv_punch_from(expected)` for the coordinator-timed
   simultaneous open.
5. **Bring up the tunnel.** On a validated `wireguard_upgrade` `TunnelInstallPlan`
   (from `validate_upgrade` / `validate_upgrade_as`,
   `crates/system/wireguard-upgrade/src/lib.rs` ~834/866), call
   `wireguard_effect::apply_tunnel_plan(&mut DefguardWireGuardEffect, ifname,
   priv_key_b64, listen_endpoint, &plan, Some(punched_reflexive))`. This is the
   **first-ever** `WGApi::configure_interface` call in the workspace.
6. **Relay fallback.** After bounded hole-punch retries + a signed
   `DirectDialFailureEvidence` (`wireguard-upgrade/src/lib.rs` ~591),
   `request_relay(peer) -> (session, relay_addr)`; re-apply the plan with
   `peer_endpoint_override = Some(relay_addr)`. Also handle the §4 relay-bind
   caveat (the coordinator must emit a dialable relay IP).
7. **Identity + runtime plumbing.** Map the node's ed25519 signer public key →
   `nat_traversal::NodeKey([u8; 32])` (raw ed25519 public key). `NatClient` is
   plain tokio; the node runs on commonware's `tokio::Runner`, so place the
   client on a dedicated task/thread — mirror the existing app-surface pattern at
   `main.rs` ~2567-2611, which already runs a plain
   `tokio::runtime::Builder::new_multi_thread()` (axum/`noded::serve`) alongside
   the runner and communicates over channels.

## §4 — The coordinator-side relay-bind fix

Verified behavior: `nat_traversal::run_coordinator` (via
`run_coordinator_with_idle`, `crates/system/nat-traversal/src/client.rs`) binds
its relay-splice sockets on the coordinator socket's **own IP**
(`bind_ip = sock.local_addr().ip()`). So `--listen 0.0.0.0:3478` produces
undialable `0.0.0.0:<port>` relay grants. STUN reflexive, rendezvous, and
hole-punch are **unaffected** (they echo/return the *observed source*).

Options for the follow-on:

- **(a) Operator binds the public IP** — documented today in
  [`coordinator.md`](coordinator.md) (the relay-bind caveat).
- **(b) Coordinator learns its public reflexive** — it already observes peers'
  source addresses, so it can infer its own routable IP and emit *that* as the
  relay-grant IP, making relay work even under a `0.0.0.0` bind.

Scope the fix to the **relay path only**; STUN/rendezvous/punch need no change.

## §5 — Why the real run needs the user's infra

The CI simulated-NAT suite (Slice 3) fakes NAT deterministically (`SimNat` drops
unsolicited inbound and rewrites source ports) and uses `FakeWireGuardEffect`
because CI has **no** WireGuard userspace runtime and **no** real NAT. The
Acceptance §2 demo therefore requires the user's boxes:

- **(a) Two hosts behind real, distinct NATs.** Cone vs. symmetric NAT behavior
  cannot be faked cross-machine, and it is exactly what decides punch-vs-relay.
- **(b) A public VPS for the coordinator** with the §4 relay-bind caveat handled.
- **(c) Real WireGuard** (`DefguardWireGuardEffect`, userspace or kernel) on both
  A and B.
- **(d) The Ducktape-2 live-join rig extended** (v2/v3 invite format, the
  NAT-hairpin gotcha, the 2-validator-quorum teardown caveat).

None of these live in CI; they are the user's infrastructure.

## §6 — Scope + handoff summary

This slice (Slice 4) ships **docs + config + a deploy proof**, and deliberately
does **not** wire the node — kept out to keep the merge gate cheap
(`cargo build -p coordinator-bin` + doc-existence checks) and to avoid the
pre-existingly-red `bin/node`/`noded` clippy from unrelated toolchain drift.

The node wiring is the enumerated **§3 checklist** — a self-contained follow-on
("Slice 5 — node reachability wiring"). Its shape: add two deps, split the config
accessor, add one orchestrator crate, drive rendezvous/punch, and make the first
`apply_tunnel_plan` effect call, plus the §4 coordinator relay fix.

**Acceptance status:**

- **§1 CI simulated-NAT suite** — DONE (Slice 3,
  `crates/system/nat-traversal/tests/simnat_ci.rs`).
- **§3 real coordinator recipe** — DONE (this slice; `coordinator --listen`
  proven by `bin/coordinator/tests/deploy_smoke.rs`). Caveat: a v3 invite can
  *address* the coordinator, but the node does not yet *use* `Coordinated` hints
  as a reachability path (§2/§3).
- **§2 cross-machine zero-exposure demo** — NOT DONE. The runbook is shipped with
  every step tagged
  ([`cross-machine-zero-exposure-runbook.md`](cross-machine-zero-exposure-runbook.md));
  it is blocked on the §3 node-wiring checklist **and** the user's real infra
  (§5). The mechanism is proven; the live tunnel is pending.
