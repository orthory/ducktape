# Private Cutover with an Untrusted Coordinator (P2+P3) — Design of Record

Status: design of record for the `epic/p3-private-cutover` epic. Combines the
coordinator role (Phase 2) and the private data-plane cutover (Phase 3) of
`docs/superpowers/specs/2026-07-04-pluggable-network-entry-design.md` into one
buildable arc, because P3 cannot run end-to-end without P2's reflexive (STUN)
service and ciphertext relay.

Companions:
- `docs/superpowers/specs/2026-07-04-pluggable-network-entry-design.md` — the
  reachability-plane design of record. This document is the detailed P2+P3
  slice of it.
- `docs/wireguard-tunnel-upgrade.md` — the validator-owned WireGuard
  **data-plane** protocol. This document does **not** edit it. The coordinator
  relay defined here lives strictly *below* WireGuard and is not a WireGuard
  `relay_candidate`.

## Goal

Reach a state where **no validator exposes a public inbound port**. A
non-validator **coordinator** provides all reachability — rendezvous, reflexive
address discovery (STUN), and, only when a direct path cannot be punched, an
opaque ciphertext relay. Steady-state data flows validator↔validator over
WireGuard end-to-end; the coordinator drops out of the data path whenever a
direct tunnel is established.

## The invariant everything rests on

The mesh transport (`commonware authenticated::discovery`) authenticates by
`ed25519` key end-to-end and is end-to-end encrypted. The WireGuard data plane
authenticates by validator identity bound to the finalized admission root.
Therefore any box on the reachability path — sentry, tunnel, coordinator,
relay — sees ciphertext only and cannot forge a validator identity. This is
what makes the coordinator safe to run as untrusted, non-validator
infrastructure.

Two load-bearing consequences:

- **No trust, only availability dependence.** A fully compromised coordinator
  can censor/delay new connections and learn coarse topology + reflexive
  addresses. It cannot decrypt, impersonate, MITM, serve state, or join
  consensus.
- **Consensus is untouched.** This epic adds no consensus change and does not
  edit the `wireguard-upgrade` "relay must be a validator" rule.

## Non-goals

- No consensus, authority, or admission change.
- No edit to the WireGuard data-plane "relay must be a validator" rule. The
  coordinator relay is a reachability-plane ciphertext splice, a different layer.
- No relaxing of the data plane into a single untrusted-relay concept (the
  "unify/relax" alternative is explicitly out per the entry design).
- No trusting the reachability plane with content or identity.
- No speculative multi-hop relay chains, relay incentives, or payment.
- No full RFC 5389 STUN; only the binding request/response subset needed for
  reflexive-address discovery.

## Components

### 1. `bin/coordinator` — the untrusted entry helper (new binary)

A non-validator process. It is **not** in the validator set, **not** in
consensus, **never** serves state, and **never** decrypts. A dedicated binary
(rather than a node mode) makes those invariants structural: coordinator code
cannot accidentally acquire validator authority. Three services:

- **Rendezvous** — a member/joiner asks to reach key `K`; the coordinator
  returns current reach information for a mesh member. It never vouches for
  `K`'s identity; the reach hint's `expected_key` does that.
- **STUN reflexive** — echoes the observer's public `ip:port` (binding
  request/response subset) so a NAT'd node learns its reflexive address to
  advertise.
- **Ciphertext relay (DERP-style)** — when two dial-out-only peers cannot
  punch a direct path, each dials out to the coordinator, which splices their
  opaque UDP packets. It never terminates the WireGuard session and never
  decrypts.

**Authorization.** The coordinator is *addressed* via the descriptor
(`Coordinated(coord_ref)`); it is not authorized as a mesh peer and is not
added to any tracked set, keeping the validator/mesh authorization model
intact. Rendezvous requests must present an inviter-signed membership/invite
token — this bounds spam and, more importantly, prevents a compromised
coordinator from substituting keys. The security rests on the signed
`expected_key`, not on the coordinator.

### 2. Typed reach hint + v3 signed invite

```text
Reach =
  | Direct(SocketAddr)        # dial addr directly (today's behavior)
  | Fronted(SocketAddr)       # dial a transport forwarder that splices to target
  | Coordinated(CoordRef)     # ask a coordinator for a path

CoordRef { coord_addr, coord_key }   # how to reach the coordinator + its channel key

ReachabilityHint {
  expected_key : ed25519 public key  # the REAL node's identity, always
  reach        : Reach
}

InviteV3 {
  network_id
  admission_root
  hints        : Vec<ReachabilityHint>
  expires
  inviter_sig  : ed25519 signature over the whole envelope
}
```

- The inviter signs the whole `InviteV3` envelope. No box on the path —
  coordinator included — can swap `expected_key` without invalidating the
  signature. This resolves the entry design's open question on whether
  rendezvous needs its own authentication: authentication lives in the invite,
  signed by the inviter.
- `Coordinated` changes dial semantics fundamentally (you dial a coordinator,
  not the target), so it warrants a real type rather than being squeezed into
  the v2 `pubkey@host:port` string. This resolves the entry design's open
  question on wire encoding in favor of a `v3` invite.
- **v2 stays parse-only** for migration: a v2 invite decodes to all-`Direct`
  hints with no signature (legacy). New invites are v3.
- Wire work lands in `bin/node/src/config.rs`, extending the current v2
  `pack`/`unpack` with a `v3` tag.

### 3. STUN client + self-endpoint discovery (new crate `crates/system/nat-traversal`)

Sends a binding request to the coordinator, learns its reflexive `ip:port`, and
publishes it into `EndpointAdvertisementV1.wireguard_endpoint` (already carried
by `wireguard-upgrade`). Reflexive addresses are global, so they satisfy the
existing advertisement endpoint parser; the flow must accept a
dynamically-discovered endpoint and re-advertise with a higher monotonic nonce
on rebinding.

### 4. UDP hole-punch (simultaneous open)

After peers exchange signed endpoint advertisements and each learns the other's
reflexive address via the coordinator, both send WireGuard handshake
initiations simultaneously (coordinator-timed) to punch NAT bindings. On
success, a direct WireGuard tunnel is established and the coordinator leaves the
data path.

### 5. WireGuard effect wiring — making the inert layer live

`wireguard-upgrade` already validates the upgrade handshake and produces a
`TunnelInstallPlan` convertible to defguard `InterfaceConfiguration`/`Peer`
values. Today nothing applies that plan: `WGApi::configure_interface` is never
called from `bin/` or `crates/kernel/`. This epic adds the effect adapter that
takes the plan and configures the interface.

The adapter sits behind a `WireGuardEffect` trait so tests use a deterministic
fake and real runs use defguard (userspace or kernel). On relay fallback the
adapter points the peer's endpoint at the coordinator relay socket instead of
the reflexive address; this is transparent to WireGuard.

### 6. Relay fallback — two relay concepts, cleanly separated

This is the crux of reconciling "zero validator exposure" with the data plane's
rules. There are two distinct relays and they must not be conflated:

- **Validator relay** — data-plane, the `wireguard-upgrade` `relay_candidates`
  mechanism, validator-only. **Unchanged by this epic.**
- **Coordinator relay** — reachability-plane, new. It does *not* use
  `relay_candidates`. Instead, on hole-punch failure the node sets the
  WireGuard peer's endpoint to a coordinator relay allocation; the coordinator
  splices ciphertext between the two sessions. Because the coordinator is a
  packet forwarder *below* WireGuard, not a WireGuard relay peer, the
  "relay must be a validator" rule is preserved intact.

Fallback is triggered by a signed, unexpired `DirectDialFailureEvidenceV1`
(already in the protocol) after bounded hole-punch retries.

## Data flow (two hidden validators A and B, happy path)

1. A and B boot dial-out-only (no inbound port). Each dials out to the
   coordinator and registers a rendezvous session, authenticated by an
   inviter-signed membership.
2. The control mesh (TCP) is already established over the entry plane (P1
   sentry / coordinator-fronted).
3. For a data tunnel, A asks the coordinator for B's reflexive endpoint (and B
   for A's). Both advertise a signed `EndpointAdvertisementV1` carrying their
   reflexive `wireguard_endpoint`.
4. The coordinator signals a simultaneous open; A and B punch; the WireGuard
   handshake completes end-to-end.
5. On success, the coordinator drops out of the data path; steady-state traffic
   is direct.
6. On failure (symmetric NAT / hairpin), each sets its WireGuard peer endpoint
   to a coordinator relay allocation; the coordinator splices ciphertext. The
   session remains end-to-end encrypted.

## Error and fallback handling

- Hole-punch timeout → relay fallback, with bounded retries and signed
  `DirectDialFailureEvidenceV1`.
- Coordinator unreachable → try the other hints in the `Vec` (multiple /
  self-hosted coordinators); already-established connections survive via
  keepalive. The coordinator is not uniquely load-bearing.
- NAT rebinding → re-run STUN and re-advertise under a higher monotonic nonce,
  respecting the duplicate-advertisement rule.
- Invite expiry or bad signature → fail closed.

## Trust and threat model

| Actor | Can | Cannot |
|-------|-----|--------|
| Coordinator (rendezvous / STUN / ciphertext relay) | Learn coarse topology + reflexive addresses, observe ciphertext + timing metadata, withhold/delay | Decrypt, impersonate, MITM, serve state, join consensus |

The one new residual risk is that a single global coordinator is a metadata
concentration point (it sees who reaches whom and when, though never content).
Mitigations, which are also design policy: allow multiple / self-hosted
coordinators in the hint `Vec`, and rely on hole-punch to keep the coordinator
out of the data path for the majority of pairs.

## Acceptance

All three of the following must hold before the epic merges to `dev`:

1. **CI simulated-NAT suite (merge gate).** An in-process harness with a fake
   NAT (drops unsolicited inbound, rewrites source port) and a fake
   `WireGuardEffect` deterministically covers: reflexive discovery; hole-punch
   success; hole-punch failure → coordinator relay splice; v3 invite signature
   verify and reject; v2 parse-compatibility; endpoint-churn re-advertisement.
   Mirrors the "Minimum Tests Before Mergeable" pattern in
   `wireguard-tunnel-upgrade.md`.
2. **Cross-machine zero-exposure demo.** Two validators on separate machines
   behind real NAT, neither exposing an inbound port, with a coordinator on a
   third box: they hole-punch a WireGuard tunnel; when hole-punch fails the
   coordinator relay carries ciphertext; real state-sync / app-hash flows over
   the tunnel. Extends the Ducktape-2 live-join rig.
   *Procedure + honest status: `docs/deploy/cross-machine-zero-exposure-runbook.md`
   (every step tagged). Blocked on node wiring — see
   `docs/deploy/private-cutover-integration-gap.md`; the CI sim-NAT suite
   (Slice 3) proves the logic.*
3. **Real coordinator standing up.** `p2p.ducktape.industries` deployed on a
   VPS with a documented deployment recipe, with a test network's v3 invite
   pointing at it.
   *Deployment recipe: `docs/deploy/coordinator.md` + `ops/coordinator/`
   (systemd unit + Dockerfile); the `coordinator --listen` invocation is
   regression-proven by `bin/coordinator/tests/deploy_smoke.rs`. Note: a v3
   invite can point at the coordinator, but the node does not consume
   `Coordinated` hints as a reachability path yet — see the gap doc.*

## Epic decomposition (thin-slice first)

The first slice proves the riskiest integration — a real WireGuard interface
coming up behind NAT — before any protocol polish. Each slice is a feature
branch off the epic branch, merged into the epic branch by PR.

- **Slice 0 — risk killer.** Minimal `bin/coordinator` (STUN reflexive +
  rendezvous only, no relay) + node STUN client + hole-punch + `WireGuardEffect`
  wiring. Prove one hidden pair forms a direct WireGuard tunnel, cross-machine.
  The coordinator reference is configured directly; no v3 invite, no relay yet.
- **Slice 1.** v3 signed invite + typed `Reach` + config wire encoding; v2
  parse-compatibility.
- **Slice 2.** Coordinator ciphertext relay + relay-fallback effect path +
  `DirectDialFailureEvidence` wiring.
- **Slice 3.** Hardening: NAT rebinding, multiple coordinators, keepalive
  survival; complete the CI simulated-NAT suite.
- **Slice 4.** Real `p2p.ducktape` deployment recipe + cross-machine acceptance
  runbook.
  *Shipped as docs + config + a deploy proof (`docs/deploy/*`,
  `ops/coordinator/*`, `deploy_smoke.rs`); node reachability wiring deferred to a
  follow-on — see `docs/deploy/private-cutover-integration-gap.md`.*

## Epic branch topology

`epic/p3-private-cutover` is a long-lived branch off `origin/dev`, worked in a
dedicated worktree. Each slice is a feature branch off the epic branch, merged
into it by PR (a reviewable unit). Once Slices 0–3 are green and the Slice 4
runbook is in place, the epic merges to `dev` via one integration PR. This
reconciles with the repo's "work targets `dev`, one PR per task" rule by
treating the epic branch as this task's integration point and `dev` as the
release target.

## Resolved open questions (from the entry design)

- **Wire encoding of the reachability hint** → a `v3` invite with a typed
  `Reach` enum; v2 remains parse-only for migration.
- **Whether coordinator rendezvous needs its own authentication** → yes, via
  inviter-signed invites; the coordinator itself remains untrusted.
