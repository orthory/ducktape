# Cross-Machine Zero-Exposure Runbook (two NAT'd validators + a coordinator)

The step-by-step for standing up two validators behind **real, distinct NATs**
(neither exposing an inbound port) plus a public coordinator, using the **real
binaries**. This is the procedure for the design's §"Acceptance" item 2. It is
written so it is unambiguous which steps run today and which remain dependent on
the two real NATs being punchable.

> **Legend.** `[WORKS TODAY]` — runs now with shipped binaries.
> `[NAT-DEPENDENT]` — the node drives the mechanism, but success depends on the
> two real NATs admitting a direct punched path. The tunnel has no relay
> fallback; the coordinator's TCP/443 lane relays only the joiner's sealed
> first-contact intro (see [`coordinator.md`](coordinator.md)).
>
> **Status update (2026-07-08).** The node-side reachability plane is wired
> behind `wireguard_listen`: `bin/node` constructs a `NatResolver` (reflexive
> discovery, `register`, hole-punch), consumes unversioned `Coordinated` hints as
> reachability routes, and desktop-created workspaces default to the public
> coordinator at `relay.ducktape.industries:3478`. The DERP-style data relay
> remains removed; the coordinator does rendezvous plus the first-contact-only
> TCP/443 relay (step 8 below).
>
> **This runbook can still fail on specific NAT pairs.** The coordinator
> rendezvouses and relays first contact only; if the two NATs will not punch,
> tunnel resolution fails honestly instead of routing peer traffic through
> the coordinator.

## Topology

Three hosts:

- **Coordinator** — a public VPS, `relay.ducktape.industries`, deployed per
  [`coordinator.md`](coordinator.md). Untrusted; no key; UDP `:3478`.
- **Validator A** — behind its own NAT, no inbound port-forward.
- **Validator B** — behind a *different* NAT, no inbound port-forward.

```
   Validator A ──┐                          ┌── Validator B
   (NAT, no      │      Coordinator         │   (NAT, no
    inbound)     └────▶ relay.ducktape.industries ◀──────┘    inbound)
                        (public VPS)
     A and B dial OUT to the coordinator; ideally they then hole-punch a
     direct A<->B WireGuard tunnel and drop the coordinator out of the path.
```

This extends the current v1 live-join rig (the NAT-hairpin gotcha and the
2-validator-quorum teardown caveat).

## Prerequisites

- The coordinator deployed and answering (per [`coordinator.md`](coordinator.md);
  verify with the Task-2 subprocess proof or `ss -lunp 'sport = :3478'`).
- Validator A and Validator B each have a built `ducktape`.
- A founder able to mint a signed, single-use bearer invite.
- For the tunnel steps: real WireGuard userspace/kernel on A and B.

## The tagged procedure

1. **Deploy the coordinator on the VPS.** `[WORKS TODAY]` — follow [`coordinator.md`](coordinator.md); confirm it binds UDP `:3478` (`ss -lunp 'sport = :3478'`) and answers a live `BindRequest` (the `deploy_smoke.rs` subprocess proof exercises exactly this).
2. **Mint an invite carrying a `Coordinated` reach hint.** `[WORKS TODAY]` — default `init` records `coordinated:<ek>@relay.ducktape.industries:3478#<coord_key>` and public coordination in `network.toml`; the one unversioned v1 blob round-trips through `bin/node/src/config/invite.rs`.
3. **A initializes; B joins and redeems its standing.** `[WORKS TODAY]` — A runs `ducktape node invite`, whose signed bearer token is the admission decision. B runs `ducktape node join <blob>` and redeems it once on first contact. If B must vote as a validator rather than remain a resident, a member then runs `ducktape node member promote <B-pubkey>`. Admission remains independent of reachability.
4. **A and B boot dial-out-only against the coordinator's reflexive/rendezvous service.** `[WORKS TODAY]` — with `wireguard_listen` configured, the node constructs a `NatResolver`, sends `BindRequest`, registers, and keeps the mapping warm.
5. **A `Coordinated` hint is consumed as a reachability path.** `[WORKS TODAY]` — `NetworkDescriptor::reach_entries()` returns `ReachDial::Coordinated`, and `bin/node` routes those entries into the reachability resolver instead of dialing the coordinator as a TCP mesh peer.
6. **A and B publish their reflexive endpoints and rendezvous.** `[NAT-DEPENDENT]` — the node drives lookup and punch through `NatResolver`; success depends on the observed NAT mappings being punchable.
7. **A and B hole-punch a direct WireGuard tunnel (coordinator-timed simultaneous open).** `[NAT-DEPENDENT]` — the library path is CI-proven and the node drives it, but a NAT pair that cannot punch fails honestly because the tunnel has no relay fallback.
8. **On hole-punch failure, resolution fails honestly.** `[BY DESIGN]` — the tunnel has no relay fallback (the DERP-style data relay was removed 2026-07-06; the coordinator's TCP/443 lane carries only the sealed first-contact intro, so a non-punching joiner can redeem its invite through it and then has no tunnel). A pair that cannot punch surfaces a `PeerFailed` or falls back only to a real advertised endpoint if one exists; a symmetric↔symmetric pair with no routable endpoint needs a different entry path.
9. **Real state-sync / root-hash flows over the tunnel.** `[NAT-DEPENDENT]` — depends on steps 6-7 establishing a direct path.

## What you CAN demo today vs. what proves the tunnel

**Today, with shipped binaries, you can:**

- Deploy the coordinator and prove it answers (the `deploy_smoke.rs` subprocess
  proof; step 1).
- Mint and parse the v1 `Coordinated` invite, and have the node consume that hint
  as coordinated reach (steps 2 and 5).
- Redeem B's resident standing and, when required, promote it (step 3).
- Register with the public coordinator and discover a coordinator-observed
  reflexive mapping (step 4).

**What still needs real infra proof:** an end-to-end tunnel across two distinct,
punchable NATs. The logic is proven at the library level by the CI simulated-NAT
suite (`crates/networking/reachability/tests/rendezvous_simnat.rs`, which drives
the production resolver over a simulated NAT topology), but the real
cross-machine run still needs two NATs that admit a direct punched path plus a
VPS coordinator.
