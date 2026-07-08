# Pluggable Network Entry — Reachability Plane — Design of Record

Status: design of record. Phase 1 (sentry) is implementable on the current
mesh with a contained change. Phases 2–3 (coordinator, private cutover) are
scoped here but deferred to their own spec→plan→impl cycles.

**Amended 2026-07-06:** the optional coordinator transport relay (the
DERP-style ciphertext splice, service 3 below) was built under the P2+P3 epic
and subsequently **removed**. The coordinator is rendezvous + STUN only; the
sole relay concept left in the system is the validator-only WireGuard
data-plane `relay_candidates` mechanism. See
`2026-07-05-private-cutover-coordinator-design.md` (amendment note) for the
rationale. The relay text below is retained as the original design record,
annotated where it no longer holds.

Companion to `docs/wireguard-tunnel-upgrade.md`. That document specifies the
validator-owned WireGuard **data-plane** tunnel-upgrade protocol. This document
specifies the orthogonal **reachability plane**: how a node reaches the mesh at
all, and how to stop forcing validators to expose a public inbound port. The two
documents describe two different layers and do not conflict; see
[Layering](#layering).

## Summary

Today a node enters the mesh exactly one way: by dialing a mesh member that
holds a reachable, advertised address. In a fresh network the only such member
is the inviter, so the inviter must keep a public inbound port open for the
invitee to reach it. This is a problem for two reasons:

1. **Exposing a validator is a risk.** A validator with a public inbound port is
   a scanning/DoS target and leaks its IP.
2. **NAT'd peers can't be reached at all.** The current transport has no NAT
   traversal and no relay, so a member behind NAT with no port-forward is
   dial-out-only and cannot serve as an entry point.

There is no hosted service to lean on, so the entry mechanism must be
**pluggable** enough to cover many deployments: a validator that self-exposes, a
validator hidden behind a sentry, a validator behind a reverse tunnel, or (later)
a coordinator/hosted broker.

The thesis of this design is to **separate reachability from authority** and put
reachability in a pluggable layer *below* the mesh's key-authenticated,
end-to-end-encrypted transport:

- **Authority plane** — who is a validator, who may serve state — is the
  finalized validator set + admission. Unchanged by this design.
- **Reachability plane** — how bytes get to a member holding a given key — is
  pluggable. Because everything on this plane only ever carries ciphertext and
  cannot forge a validator identity, its participants (sentries, tunnels,
  coordinators, relays) **need not be validators and need not be trusted.**

One abstraction — a *reachability hint* `(expected_key, reach)` — expresses
"direct validator", "sentry-fronted validator", "tunnel-fronted validator", and
(future) "coordinator-brokered entry" interchangeably, chosen per deployment.

## The invariant everything rests on

The mesh transport is commonware `authenticated::discovery`: an **encrypted,
key-authenticated** TCP mesh. Two consequences are load-bearing for this entire
design:

- **Authentication is by key, not by IP.** A dialer dials an address and expects
  a specific `ed25519` public key; the handshake authenticates that key
  end-to-end. Whatever network path delivered the bytes is irrelevant to
  authenticity.
- **The channel is end-to-end encrypted.** Anything sitting on the path between
  two endpoints sees ciphertext only.

Therefore any box on the reachability path is, at most, a transparent
ciphertext forwarder that can drop or delay traffic (an availability/censorship
actor) but **cannot read content and cannot impersonate a peer.** That is what
makes the reachability plane safe to make pluggable and safe to populate with
non-validator, untrusted infrastructure.

## Layering

The reconciliation with `wireguard-tunnel-upgrade.md` is strict layer
separation. Its hard non-goal — "no external relay that is not also in the
validator set" — governs the **WireGuard data-plane relay**: a mesh participant
that forwards authenticated WireGuard traffic and could otherwise be treated as a
serving peer. That rule is retained unchanged.

```
┌───────────────────────────────────────────────────────────┐
│ Authority: validator set + admission        (UNCHANGED)    │
├───────────────────────────────────────────────────────────┤
│ WireGuard data-plane relay = validator-only (UNCHANGED,    │
│   docs/wireguard-tunnel-upgrade.md)          existing doc) │
├───────────────────────────────────────────────────────────┤
│ mesh crypto: key-auth + E2E encryption      (the invariant)│
├───────────────────────────────────────────────────────────┤
│ Reachability plane (THIS DOC):              non-validator  │
│   sentry / reverse-tunnel / coordinator      OK — ciphertext│
│                                              only, untrusted│
└───────────────────────────────────────────────────────────┘
```

The sentry/tunnel/coordinator of the reachability plane operate *below* mesh
crypto. They are never a mesh participant, never serve state, never decrypt.
They are invisible to the mesh — the mesh only ever sees an authenticated
connection to the real key. They are therefore **out of scope of the WireGuard
data-plane relay non-goal**, and this design does not edit that document. This
document only cross-references it.

## Core abstraction: the reachability hint

Generalize the bootstrap entry from "a validator's own advertised address" to a
**reachability hint**:

```text
ReachabilityHint {
  expected_key : ed25519 public key    # the REAL node's identity, always
  reach        : Reach                 # how to obtain a byte-stream to it
}

Reach =
  | Direct(addr)              # dial addr directly (today's behavior)
  | Fronted(addr)            # dial addr; a transport forwarder splices to target
  | Coordinated(coord_ref)   # ask a coordinator for a path (future, Phase 2+)
```

Properties:

- `expected_key` is always the real node's key. The mesh handshake authenticates
  end-to-end against `expected_key` regardless of how the bytes arrived. A wrong
  or malicious forwarder can withhold or DoS, but **cannot impersonate.**
- `Direct` and `Fronted` are wire-identical from the mesh's point of view — both
  are "dial this address, expect this key." `Fronted` is therefore representable
  on the current mesh with no protocol change; the address is simply the
  sentry's. The distinction is carried for operator clarity and to leave a clean
  seam for `Coordinated`.
- Hints are a set (already a `Vec` in `NetworkDescriptor::bootstrap`), and a
  descriptor may carry several nodes' hints. A joiner tries them in turn.
  Resilience comes from having N entry points, none of them uniquely
  load-bearing.

## Phase 1 — Sentry (deployable on the current mesh)

A **sentry** is a transparent TCP forwarder placed in front of a validator so
the validator never exposes a public inbound port.

Two deployment styles, identical to the mesh:

- **Forward sentry (Cosmos-style):** the sentry listens on a public address and
  forwards to the validator's private listen address. Requires
  sentry→validator reachability (shared private network / firewall exception).
  Realizable with `nginx stream`, HAProxy TCP mode, or a small Rust forwarder.
- **Reverse tunnel:** the validator dials *out* to an edge (frp, rathole,
  `ssh -R`, cloudflared-style), and the edge is the public face. No inbound port
  on the validator at all. The "sentry address" is the tunnel edge.

Configuration in both styles:

- The validator sets its advertised address to the **sentry/edge address**
  (`advertised = <sentry_addr>`), while its real `listen` address stays private.
  `advertised` is already independent of `listen` in `bin/node/src/config.rs`.
- Peers dial the sentry address; the sentry splices to the validator; the mesh
  handshake terminates at the validator through the pipe.

Why this restores reachability without exposure: a validator that advertises no
reachable address is dial-out-only (others cannot reach it). The sentry gives it
a dialable public address it does not itself have to bind — the validator's real
endpoint is never advertised.

What the sentry is NOT: it is not a mesh member, not a validator, and it does not
serve state. State-sync is still served only by validators
(`choose_sync_source` requires a validator that is not self). The sentry is only
the *path* to a validator; a joiner's state-sync still terminates at a validator
through the sentry pipe.

Protections gained:

- The validator has no public inbound port; the sentry absorbs scanning of the
  validator's real IP.
- Multiple sentries per validator give redundancy; sentries rotate without
  touching the validator's key.

Phase-1 limitation — source-IP collapse. A transparent forward splice (or reverse
tunnel) makes the validator's listener observe **every peer from one source IP**:
the sentry's. commonware keys its private-IP gate and its per-IP / per-subnet
handshake rate limits on that observed IP, so under fronting those defenses no
longer discriminate per real peer — one abusive dialer through the sentry can
exhaust the shared handshake budget, and a validator restart re-handshakes inbound
peers through a single bucket. Handshake-layer DoS absorption therefore holds only
for a **filtering** sentry, not a transparent splicer. Mitigations (later work):
expose and relax the per-IP/subnet handshake quotas on fronted validators (the
node currently hardcodes the `local` preset), and/or a PROXY-protocol-aware sentry
plus commonware PROXY-header parsing to preserve the real source IP (neither
exists today). See `docs/sentry-deployment.md`.

Expected code delta (to be confirmed by the Phase 1 investigation):

- Represent the reachability hint explicitly (`Direct`/`Fronted`) or, at minimum,
  confirm and document that `advertised = sentry_addr` already produces correct
  behavior end-to-end.
- Guardrails so a legitimately-fronted validator does not trip a
  "advertised != listen" warning or validation, and so a joiner selecting a
  state-sync source behind a sentry behaves correctly.
- An integration test that stands up a sentry (TCP forwarder) in front of a
  validator and proves a joiner is admitted and syncs through it — converting an
  implicit capability into a supported, regression-guarded feature.
- Deployment recipes (forward sentry + reverse tunnel).

Target: no consensus change.

## Phase 2 — Coordinator (non-validator entry helper)

A **coordinator** is a non-validator node that provides entry services only:

1. **Rendezvous** — help a joiner find a current mesh member to bootstrap from.
2. **Reflexive address (STUN-style)** — tell a NAT'd node its observed public
   `ip:port` so it can advertise it and attempt a direct data tunnel.
3. ~~**Transport relay (optional)** — a ciphertext splice used when direct
   dialing fails; the reachability-plane equivalent of a DERP relay.~~
   *Removed 2026-07-06 (see the amendment note): the coordinator never
   carries peer traffic.*

The coordinator is not in the validator set, not in consensus, never serves
state, and never decrypts. It generalizes the "hosted service" role so that
anyone can run it — a member's own box, a cheap VPS, or later
`ducktape.industries`. The design is deployment-agnostic: it defines the role
and protocol, not who operates it.

Authorization-model impact (kept minimal): a coordinator is **addressed** by
joiners via the descriptor (`Coordinated(coord_ref)`); it is **not** authorized
as a mesh peer and is **not** added to any tracked set. This keeps the
validator/mesh authorization model intact — the coordinator lives outside it.

## Phase 3 — Private cutover (Tailscale model)

Two transports with different NAT-traversal stories:

- **Entry/admission runs over TCP** (commonware `authenticated::discovery`). TCP
  reachability is solved by fronting/relay (Phases 1–2), not hole-punching.
- **Steady-state data runs over WireGuard/UDP.** UDP hole-punching is viable.

Cutover sequence, after admission over the entry plane:

1. Peers exchange **signed endpoint advertisements** (the existing
   `EndpointAdvertisement` in `wireguard-upgrade`).
2. Attempt a **direct WireGuard** tunnel using either a static reachable UDP
   endpoint (if the node has one) or a **UDP hole-punch** coordinated via the
   coordinator's reflexive-address service plus simultaneous-open.
3. On failure, emit signed `DirectDialFailureEvidence` and fall back to the
   **WireGuard data-plane relay** — validator-only, per
   `wireguard-tunnel-upgrade.md`. *(The original design listed a second,
   coordinator-relay flavor here; it was built and then removed 2026-07-06 —
   at the reachability plane, a failed punch is now terminal and surfaced.)*

This phase wires the currently-inert effect layer
(`WGApi::configure_interface`) and delivers the missing NAT-traversal primitive
(self-endpoint discovery / STUN client). After cutover the data plane is private
and direct, and the coordinator drops out of the data path.

## Trust and threat model

| Actor | Can | Cannot |
|-------|-----|--------|
| Sentry / reverse tunnel | Forward ciphertext, observe traffic metadata, withhold/delay | Read content, impersonate a peer, serve state, join consensus |
| Coordinator (rendezvous / STUN) | Learn coarse topology + reflexive addresses, withhold service | Read content, impersonate, **MITM** |

"Free tailnet lock": endpoint and WireGuard-key advertisements are signed by the
validator's `ed25519` identity and bound to the finalized admission root. A
broker cannot inject its own key to man-in-the-middle a tunnel — the mechanism
Tailscale bolts on as an optional "tailnet lock" is intrinsic here because keys
are bound to admitted validator identities.

Residual cost, stated honestly: any entry helper is an **availability and
censorship dependency for new connections**. It is mitigated by (a) multiple
independent entry points, (b) self-hosting, (c) falling back to direct or to a
different coordinator, and (d) the fact that already-established connections
survive coordinator downtime via keepalive.

This model is strictly weaker-trust than making the relay a validator: a
validator relay carries consensus authority, whereas a reachability-plane relay
carries none. The existing "relay must be a validator" rule is thus a
self-sovereignty choice for the data plane, not a security necessity for the
reachability plane. *(With the coordinator relay removed, this weaker-trust
argument is historical: the validator-only rule now describes every relay in
the system.)*

## Roadmap

| Phase | Deliverable | Consensus change | Deployable |
|-------|-------------|------------------|------------|
| P0 | This design of record | none | n/a |
| P1 | Sentry: configuration-only fronting confirmed + integration test + deployment recipes (typed reach hint / guardrails deferred — not needed; see the Phase 1 plan) | none | now |
| P2 | Coordinator role: rendezvous + STUN reflexive service (relay built, then removed 2026-07-06) | authorization-model addition (addressed, not tracked) | partial |
| P3 | Private cutover: WGApi wiring + UDP hole-punch (punch failure is terminal — no relay fallback) | data-plane wiring | staged |

Each phase is independently useful and gets its own spec→plan→impl cycle.

## Non-goals

- No change to authority, consensus, or admission.
- No edit to the WireGuard data-plane "relay must be a validator" rule (layer
  separation).
- No trusting the reachability plane with content or identity.
- Not building a global hosted service now — the coordinator role is defined;
  who operates it is a deployment decision.
- No speculative multi-hop relay chains, relay incentives, or payment.

## Open questions

- Exact wire encoding of the reachability hint (does it warrant a `v3` invite
  format, or an additive field on `v2`?) — resolved in the Phase 1 plan.
- Whether coordinator rendezvous needs its own authentication (e.g., signed by an
  inviter) — Phase 2.
- The "unify/relax" alternative (a single untrusted-relay concept spanning both
  layers, relaxing the data-plane validator-only rule) is explicitly **out** for
  this design; layer separation was chosen.

## Code anchors

- `bin/node/src/config.rs` — `NetworkDescriptor`, `bootstrap`, `add_bootstrap`,
  `bootstrap_entries`, `dialable`, `advertised`/`listen` resolution.
- `bin/node/src/main.rs` — `discovery::Config::local`, `advertised` flow,
  `choose_sync_source` (validator-only state-sync).
- `crates/system/wireguard-upgrade/src/lib.rs` — `EndpointAdvertisement`, relay
  candidates, `DirectDialFailureEvidence`, `TunnelInstallPlan` (inert leaf).
- `crates/system/valset-mesh-interface/src/lib.rs` — mesh roles (bootnode/relay
  capabilities, `requires_external_relay`).
- `docs/wireguard-tunnel-upgrade.md` — the data-plane tunnel-upgrade protocol
  this design complements.
