# Private Cutover — reachability integration: current state

This document tracked the gap between the proven reachability library crates
and the live node. The node wiring has since shipped; this is the surviving
map of what runs today and what remains. History (the original §1–§6 gap
analysis) lives in git.

## What runs today

**Two planes, composed orthogonally** (unchanged decision):

- The **control mesh** is commonware `authenticated::discovery` over TCP,
  key-authenticated end to end.
- The **data tunnel** is validator↔validator WireGuard, driven by the
  `reachability` orchestrator crate: per-epoch record gossip → signed
  advertisements → `MeshView::verify` → pairwise handshakes → one
  `apply_tunnel_plans` per epoch, with `NatResolver`
  (STUN/rendezvous/punch via the coordinator; no relay — a failed punch is
  terminal) resolving each peer's UDP endpoint.

**Transitive gossip.** Reachability-plane messages no longer assume pairwise
transport: records/adverts flood with nonce dedup, handshake messages fan to
every peer and relay through stage-superseding per-pair slots, and every
message authenticates by its owner's content signature
(`SignedEndpointRecord`, public `verify_signature` on adverts and the
handshake triple) — never by the delivering link. A joiner with a single
ingress link (an ephemeral fronted hint) assembles tunnels with every member
through that one link.

**Mesh over the tunnels.** `advertised = "overlay"` makes a node advertise
its chain-derived overlay ULA (`ula_v6_member_addr`) at its mesh listen port
(requires `listen = "[::]:<port>"`). Discovery gossip distributes it; dialers
retry until the tunnel makes it routable. Proven by the container smoke: two
real-WireGuard nodes advertising overlay keep finalizing after the underlay
TCP path is cut in both directions. A `Coordinated` reach hint routes the
WireGuard path through its coordinator; the overlay advertisement is how the
TCP mesh follows.

**Registration → online → activation.** Validator membership is two-phase:
governance `Join` registers a key as STANDBY (transport-tracked,
statesync-served, quorum-exempt; arms a transport-only cutover). The parked
node probes its registration from the valset snapshot, proves a full state
sync, and announces ONLINE over the lobby with its own height-windowed
signature; any active member relays the proof as `ValsetMsg::Online`
(re-verified in-module), and the activation cutover widens the quorum. A
registered-but-absent node costs consensus nothing.

## The join recipe (tunnel-first invite — no TCP ingress at all)

The throwaway `fronted:` TCP hint is gone: the invite blob IS the VPN
credential, and the join window's carrier is the invite tunnel itself.

1. A member runs `invite`: the signed blob carries the network descriptor,
   the member's WireGuard public key + underlay UDP endpoint, its UDP intro
   endpoint (`invite_listen`, default WG port + 1), its overlay mesh port,
   an expiry, and a single-use token — minting IS the admission decision.
2. `join <blob>` writes the workspace with WireGuard-shape defaults (own
   plane, dual-stack mesh listen, `advertised = "overlay"`, the inviter's
   overlay ULA as a Direct dial hint) and the node starts: it installs the
   inviter as a join-window tunnel peer straight from the blob, announces
   its identity + WireGuard key to the intro listener (one
   token-authenticated datagram; the inviter installs the peer and acks),
   and the tunnel comes up — before any p2p.
3. The mesh dials the inviter's overlay ULA the moment the tunnel routes;
   the joiner's lobby announce rides it, and the receiving member submits
   the governance `Redeem` op automatically — no approval verb. The grant
   cutover re-tracks the mesh, the pre-warm layer assembles tunnels with
   every member, and the joiner syncs (rotating across every serving
   validator) into a serving FULL NODE.
4. Seating it in the quorum stays a separate, deliberate act (`promote`) —
   the existing standby → online → activation machinery unchanged.

**Fully-NATed inviter — ADDRESSED (unified all-paths invite + fronts;
coordinator ambient).** The blob is no longer just the inviter's own paths: it
bundles EVERY entry the inviter offers as ONE candidate set — `{inviter} ∪
{fronts}`, where each front is a reachable member the inviter already meshes
with (read from persisted `mesh-state.json` at mint), carrying that member's WG
key, mesh port, and its underlay endpoint when it has one (else `None`, reached
by identity). The joiner races the whole union: a candidate with an endpoint is
dialed directly and announced at `wg_port + 1`; an endpoint-less one is driven
through `BootstrapCoordinatedInvitePeer` (rendezvous → install → intro over the
punched socket). First `IntroAck.installed` wins, the rest are cancelled, and
exhaustion is an HONEST terminal (a distinct non-zero exit with a mode-naming
FATAL — never a silent success). So a fully-NATed (incl. symmetric) inviter
still onboards as long as ONE offered path — itself via its coordinator, or any
reachable front — comes up. The coordinator is AMBIENT: the joiner's resolver
binds `config::primary_coordinator_or_default` (public default), never an
address baked into the blob; `invite` embeds no coordinator address at all.
`fronts` live OUTSIDE the genesis fingerprint (advisory reachability, not
validator identity — proven by a fingerprint-exclusion test), so this is ZERO
consensus change. No coordinator data relay (that was removed 2026-07-06, and
the design here needs none).

Deliberate bounds, for now: a DIRECT candidate's intro listener is the peer's
own UDP (`wg_port + 1`), so a direct path needs that member's port
underlay-reachable (one forwarded UDP port suffices; the joiner needs nothing) —
but a coordinated path needs no forwarded port at all. Under a real kernel-TUN
effect the userspace rendezvous is inactive, so coordinated candidates are
dropped from the race (an all-coordinated invite on a TUN node then hits the
honest terminal immediately rather than hanging).

The two-node real-WireGuard container smoke that proved mesh-over-tunnels
(and its cold-restart leg) lives at `ops/wg-smoke/run-smoke.sh`; extending
it to drive this join recipe end-to-end on real tunnels is the standing
verification gate for the tunnel-first flow (the TCP-carrier halves are
proven by `bin/node/tests/join_request_e2e.rs`).

## Cold restart (shipped)

A member restarting with zero TCP links (ingress gone, tunnels torn down at
exit) — and the whole-network cold start — heals from disk. Every applied
epoch persists its verified advert set (`<storage>/mesh-state.json`,
signature-verified on load); the first boot `Retarget` re-applies that mesh
purely locally — peer WireGuard keys and advertised endpoints from the
persisted records, overlay ULAs re-derived from `(chain_id, identity)`,
NATed endpoints re-resolved FRESH through the coordinator (`NatResolver`
needs no gossip; one-sided resolution suffices because WireGuard roams on
authenticated inbound) — and the dialer is seeded with the persisted control
ULAs for peers without a configured hint. The restored mesh is purely a
gossip carrier: the boot epoch's own live assembly replaces it at its apply.
What it deliberately does NOT cover: the FIRST join on a coordinated-only
config (nothing persisted yet — the throwaway fronted hint in the join
recipe above remains required for the join window).

## Standby tunnel pre-warming (shipped)

The plane's epochs still version ACTIVE members only (phase A's all-members
rule stands — a registered-but-absent key must never stall an epoch), but
the resident tier now rides a separate PRE-WARM layer with the opposite
trade: never versioned, never handshaked, applied live. Every `Retarget`
carries the epoch's resident set as `standbys`; a standby's owner-signed
`EndpointRecord` (bound to the epoch tuple, policy-checked, nonce-superseded)
installs a tunnel by re-applying the full interface config in place — the
same record-derived trust model as the cold-restart restore — and a
higher-nonce re-advertisement moves its endpoint live. The parked joiner
runs the plane in the standby role off its manifest polls (its gossip is
admitted under the derived lobby identity; replies route back over the
delivering transport key), installs every member on its own interface, and
persists the member adverts — so the promotion reboot restores full
connectivity from disk before the new epoch's phase-A assembly replaces it
with the verified mesh. Result: a joiner's tunnels exist BEFORE its
activation cutover, not seconds after.

## Rendezvous hardening (shipped 2026-07-07)

The rendezvous plane now holds up over real time and real NATs, three ways.
Coordinator registrations EXPIRE (`REGISTRATION_TTL_SECS`, 120 s): an expired
key resolves to an honest `None` and receives no `PunchSync` toward its dead
pinhole, and the nonce anti-rollback guard yields for corpses so a rebooted
node re-registers cleanly. Nodes hold their mapping with a keepalive
`Readvertise` every `RENDEZVOUS_KEEPALIVE` (25 s) from the resolver's pump
task — the same datagram keeps the NAT pinhole open, and its wall-clock-seeded
nonce supersedes the node's own pre-reboot mapping. And the punch no longer
needs both sides to resolve simultaneously: the pump answers
coordinator-vouched `PunchSync` fan-outs while the node is otherwise idle, and
the active side re-`Lookup`s on every punch retry (each one re-fans the sync),
so one-sided resolution completes against an idle-but-alive peer. What still
moves with the userspace-overlay ADR's phase 3 is the punched-pinhole ↔
WireGuard-socket alignment (the punch must originate from the plane's own UDP
socket) and reflexive-bearing endpoint advertisements.

## What remains (this seam's follow-ons)

1. **Coordinator-auth (§3.5 of the original doc).** The join window's first
   contact is now token-authenticated end to end (the intro datagram carries
   the inviter-signed token + the joiner's proofs), but COORDINATOR
   rendezvous messages still carry no token; a compromised coordinator can
   deny service (never substitute keys — records pin WireGuard keys under
   the owner's ed25519 signature). Extending the intro's token discipline to
   the rendezvous remains open. (A coordinator-RELAYED intro for a
   fully-NATed inviter is NO LONGER needed — the unified all-paths invite
   above onboards a fully-NATed inviter through its ambient coordinator and
   its reachable fronts, with no relay.)

(The former item 2 — the relay-bind caveat — dissolved when the DERP-style
relay was removed on 2026-07-06: a wildcard-bound coordinator is now fully
functional, since every answer derives from the datagram's observed source.)
