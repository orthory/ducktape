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
  (STUN/rendezvous/punch/relay via the coordinator) resolving each peer's
  UDP endpoint.

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
   punched/relayed via the coordinator); the mesh dials its overlay ULA.
4. The ingress can die: mesh traffic rides the tunnels.

## What remains (this seam's follow-ons)

1. **Cold assembly / restart.** A NATed member restarting with zero TCP links
   (ingress gone, tunnels torn down) has no path for plane gossip. Fix
   sketch: persist the verified mesh (records + tunnel plans) and re-apply at
   boot with fresh coordinator-resolved endpoint overrides, seeding the
   dialer from the persisted control ULAs. Until then: keep one fronted hint
   alive, or restart members one at a time (live peers re-establish tunnels
   to a restarted member via relayed gossip).
2. **Standby tunnel pre-warming.** The reachability plane's epochs cover
   ACTIVE members only: phase A's all-members rule means a registered-but-
   absent standby key would stall every epoch's bring-up, so standby nodes
   join the plane at activation instead — tunnels assemble in the seconds
   after the activation cutover (relayed gossip + coordinator punch), not
   before. Pre-warming standby tunnels wants phase B (live re-advertisement
   / partial-mesh applies).
3. **Coordinator-auth (§3.5 of the original doc).** Rendezvous messages still
   carry no inviter-signed token; a compromised coordinator can deny service
   (never substitute keys — records pin WireGuard keys under the owner's
   ed25519 signature). The token design remains open.
4. **Relay-bind caveat.** A coordinator behind `0.0.0.0` must advertise a
   routable relay IP — `run_coordinator_advertised` exists; operator guidance
   in [`coordinator.md`](coordinator.md).
