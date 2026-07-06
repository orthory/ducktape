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

## The join recipe (coordinated-only invite + ephemeral ingress)

1. Inviter mints `coordinated:<key>@<coordinator>#<coord_key>` plus one
   throwaway `fronted:` hint (any TCP ingress that reaches it — the join
   window only).
2. Joiner parks through the ingress, delivers its key over the lobby;
   `invite-accept` registers it standby.
3. The joiner syncs, announces online, activates; the reachability plane
   brings up its tunnels (gossip relayed through the ingress link, WireGuard
   punched via the coordinator's rendezvous); the mesh dials its overlay ULA.
4. The ingress can die: mesh traffic rides the tunnels.

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
the observer tier now rides a separate PRE-WARM layer with the opposite
trade: never versioned, never handshaked, applied live. Every `Retarget`
carries the epoch's observer set as `standbys`; a standby's owner-signed
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

## What remains (this seam's follow-ons)

1. **Coordinator-auth (§3.5 of the original doc).** Rendezvous messages still
   carry no inviter-signed token; a compromised coordinator can deny service
   (never substitute keys — records pin WireGuard keys under the owner's
   ed25519 signature). The token design remains open.

(The former item 2 — the relay-bind caveat — dissolved when the DERP-style
relay was removed on 2026-07-06: a wildcard-bound coordinator is now fully
functional, since every answer derives from the datagram's observed source.)
