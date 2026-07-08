# Private Cutover with an Untrusted Coordinator (P2+P3) — Design of Record

Status: design of record for the `epic/p3-private-cutover` epic. Combines the
coordinator role (Phase 2) and the private data-plane cutover (Phase 3) of
`docs/superpowers/specs/2026-07-04-pluggable-network-entry-design.md` into one
buildable arc, because P3 cannot run end-to-end without P2's reflexive (STUN)
service.

**Amended 2026-07-06: the DERP-style ciphertext relay is REMOVED.** The
coordinator is now rendezvous-only — STUN reflexive discovery + hole-punch
brokering. It never binds a data socket and never carries peer traffic, so no
data path ever depends on it. A pair that cannot hole-punch (symmetric NAT on
both sides) terminally fails resolution and rides its advertised endpoint,
surfaced as a `PeerFailed` observability event — an honest failure instead of a
silent coordinator-carried degradation. Slice 2 (the relay) shipped and was
subsequently retired; its wire tags (8/9) stay reserved so stale peers decode
`BadTag` instead of aliasing future messages.

Companions:
- `docs/superpowers/specs/2026-07-04-pluggable-network-entry-design.md` — the
  reachability-plane design of record. This document is the detailed P2+P3
  slice of it.
- `docs/wireguard-tunnel-upgrade.md` — the validator-owned WireGuard
  **data-plane** protocol. This document does **not** edit it; in particular
  its validator-only `relay_candidates` mechanism is untouched (and is the
  only relay concept left in the system).

## Goal

Reach a state where **no validator exposes a public inbound port**. A
non-validator **coordinator** provides rendezvous and reflexive address
discovery (STUN); direct paths are then hole-punched. Steady-state data flows
validator↔validator over WireGuard end-to-end; the coordinator is never on the
data path.

Scope note: with no relay, "zero inbound exposure" holds for every NAT pair
hole-punch can traverse (full-cone through port-restricted cone). A
symmetric↔symmetric pair does not connect through the coordinator — it needs a
routable endpoint on at least one side, or the in-mesh member relaying of the
reachability plane's gossip layer for control traffic. This is a deliberate
trade: the relay bought symmetric-NAT coverage at the cost of a
coordinator-carried data path, an always-on data-plane availability dependence,
and a standing metadata concentration point.

## The invariant everything rests on

The mesh transport (`commonware authenticated::discovery`) authenticates by
`ed25519` key end-to-end and is end-to-end encrypted. The WireGuard data plane
authenticates by validator identity bound to the finalized admission root.
Therefore any box on the reachability path — sentry, tunnel, coordinator —
sees ciphertext only and cannot forge a validator identity. This is what makes
the coordinator safe to run as untrusted, non-validator infrastructure.

Two load-bearing consequences:

- **No trust, only availability dependence — and only at rendezvous time.** A
  fully compromised coordinator can censor/delay NEW connection setup and learn
  coarse topology + reflexive addresses. It cannot decrypt, impersonate, MITM,
  serve state, join consensus, or touch an established tunnel (no data path
  traverses it).
- **Consensus is untouched.** This epic adds no consensus change and does not
  edit the `wireguard-upgrade` "relay must be a validator" rule.

## Non-goals

- No consensus, authority, or admission change.
- No edit to the WireGuard data-plane "relay must be a validator" rule.
- **No DERP-style coordinator relay.** The coordinator never splices, forwards,
  or observes data-plane ciphertext. (An earlier revision shipped one; removed
  2026-07-06 — see the amendment note.)
- No relaxing of the data plane into a single untrusted-relay concept (the
  "unify/relax" alternative is explicitly out per the entry design).
- No trusting the reachability plane with content or identity.
- No full RFC 5389 STUN; only the binding request/response subset needed for
  reflexive-address discovery.

## Components

### 1. `bin/coordinator` — the untrusted entry helper (new binary)

A non-validator process. It is **not** in the validator set, **not** in
consensus, **never** serves state, and **never** decrypts. A dedicated binary
(rather than a node mode) makes those invariants structural: coordinator code
cannot accidentally acquire validator authority. Two services:

- **Rendezvous** — a member/joiner asks to reach key `K`; the coordinator
  returns current reach information for a mesh member and fans a `PunchSync`
  to both sides. It never vouches for `K`'s identity; the reach hint's
  `expected_key` does that.
- **STUN reflexive** — echoes the observer's public `ip:port` (binding
  request/response subset) so a NAT'd node learns its reflexive address to
  advertise.

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
path entirely.

### 5. WireGuard effect wiring — making the inert layer live

`wireguard-upgrade` already validates the upgrade handshake and produces a
`TunnelInstallPlan` convertible to defguard `InterfaceConfiguration`/`Peer`
values. Today nothing applies that plan: `WGApi::configure_interface` is never
called from `bin/` or `crates/kernel/`. This epic adds the effect adapter that
takes the plan and configures the interface.

The adapter sits behind a `WireGuardEffect` trait so tests use a deterministic
fake and real runs use defguard (userspace or kernel). A punched path is wired
in through `peer_endpoint_override` — the peer's endpoint is pointed at the
hole-punch's resolved reflexive address instead of the statically advertised
one; this is transparent to WireGuard.

### 6. Hole-punch failure is terminal — no relay fallback

There is exactly ONE relay concept in the system:

- **Validator relay** — data-plane, the `wireguard-upgrade` `relay_candidates`
  mechanism, validator-only. **Unchanged by this epic.**

When hole-punch fails after bounded retries, endpoint resolution FAILS: the
peer rides its statically advertised endpoint and the orchestrator emits a
`PeerFailed` event for observability. Nothing degrades onto a
coordinator-carried path, because none exists. The retired coordinator-relay
alternative (an opaque below-WireGuard UDP splice) is documented in this file's
git history and in `docs/superpowers/plans/2026-07-05-slice2-coordinator-relay.md`
(historical record).

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
5. On success, the coordinator is out of the picture; steady-state traffic is
   direct.
6. On failure (symmetric NAT / hairpin), resolution fails honestly: the peer
   rides its advertised endpoint and a `PeerFailed` surfaces the pair for
   operator action (give one side a routable endpoint, or accept the pair
   stays mesh-relayed at the control layer).

## Error and fallback handling

- Hole-punch timeout → bounded retries, then terminal resolution failure
  (`PeerFailed` + advertised endpoint). No relay fallback.
- Coordinator unreachable → try the other hints in the `Vec` (multiple /
  self-hosted coordinators); already-established connections survive via
  keepalive. The coordinator is not load-bearing for any established path.
- NAT rebinding → re-run STUN and re-advertise under a higher monotonic nonce,
  respecting the duplicate-advertisement rule.
- Invite expiry or bad signature → fail closed.

## Trust and threat model

| Actor | Can | Cannot |
|-------|-----|--------|
| Coordinator (rendezvous / STUN) | Learn coarse topology + reflexive addresses, observe rendezvous timing metadata, withhold/delay new connection setup | Decrypt, impersonate, MITM, serve state, join consensus, observe or affect established tunnels |

The residual metadata risk shrank with the relay's removal: the coordinator
sees who *rendezvouses* with whom, but never a data path, its volume, or its
timing. Mitigations, which are also design policy: allow multiple /
self-hosted coordinators in the hint `Vec`; every established pair is fully
coordinator-independent.

## Acceptance

All three of the following must hold before the epic merges to `dev`:

1. **CI simulated-NAT suite (merge gate).** An in-process harness with a fake
   NAT (drops unsolicited inbound, rewrites source port) and a fake
   `WireGuardEffect` deterministically covers: reflexive discovery; hole-punch
   success; hole-punch failure → terminal `NotReachable` (no fallback); v3
   invite signature verify and reject; v2 parse-compatibility; endpoint-churn
   re-advertisement. Mirrors the "Minimum Tests Before Mergeable" pattern in
   `wireguard-tunnel-upgrade.md`.
2. **Cross-machine zero-exposure demo.** Two validators on separate machines
   behind real (punchable) NAT, neither exposing an inbound port, with a
   coordinator on a third box: they hole-punch a WireGuard tunnel; real
   state-sync / app-hash flows over the tunnel. Extends the Ducktape-2
   live-join rig.
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
  rendezvous only) + node STUN client + hole-punch + `WireGuardEffect`
  wiring. Prove one hidden pair forms a direct WireGuard tunnel, cross-machine.
  The coordinator reference is configured directly; no v3 invite.
- **Slice 1.** v3 signed invite + typed `Reach` + config wire encoding; v2
  parse-compatibility.
- **Slice 2.** ~~Coordinator ciphertext relay + relay-fallback effect path +
  `DirectDialFailureEvidence` wiring.~~ *Shipped, then retired 2026-07-06 —
  the relay was removed and the coordinator returned to the Slice 0 shape
  (rendezvous only). Historical record:
  `docs/superpowers/plans/2026-07-05-slice2-coordinator-relay.md`.*
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
into it by PR (a reviewable unit). The epic merges to `dev` via one integration
PR **only when all three Acceptance items above hold** — not merely when the
Slice 4 runbook lands. Landing the docs/config/runbook (Slice 4) satisfies
Acceptance §3 and *documents* §2, but Acceptance §2 (the real cross-machine
zero-exposure demo) is still open: it is blocked on the node-side reachability
wiring the node does not yet have — `NatClient` reflexive discovery, hole-punch,
WireGuard-effect bring-up, and the §3.5 coordinator-auth work — enumerated in
`docs/deploy/private-cutover-integration-gap.md` and gated on the user's real
NAT/VPS/WireGuard infra. So the runbook being "in place" is a **necessary, not
sufficient** condition for merge; the epic is merge-ready only once §2 also
passes. This reconciles with the repo's "work targets `dev`, one PR per task"
rule by treating the epic branch as this task's integration point and `dev` as
the release target.

## Resolved open questions (from the entry design)

- **Wire encoding of the reachability hint** → a `v3` invite with a typed
  `Reach` enum; v2 remains parse-only for migration.
- **Whether coordinator rendezvous needs its own authentication** → yes, via
  inviter-signed invites; the coordinator itself remains untrusted.
- **Symmetric-NAT coverage** → dropped from scope with the relay's removal
  (2026-07-06): a symmetric↔symmetric pair needs a routable endpoint on one
  side; the system surfaces the failure instead of relaying.
