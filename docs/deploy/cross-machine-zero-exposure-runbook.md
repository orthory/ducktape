# Cross-Machine Zero-Exposure Runbook (two NAT'd validators + a coordinator)

The step-by-step for standing up two validators behind **real, distinct NATs**
(neither exposing an inbound port) plus a public coordinator, using the **real
binaries**. This is the procedure for the design's §"Acceptance" item 2. It is
written so it is unambiguous which steps run today and which are blocked on node
wiring.

> **Legend.** `[WORKS TODAY]` — runs now with shipped binaries.
> `[NEEDS NODE WIRING]` — the mechanism exists and is CI-proven in
> `crates/system/nat-traversal` and/or `crates/system/wireguard-effect`, but
> `ducktape-node` does **not** call it yet (`nat-traversal` and
> `wireguard-effect` are not dependencies of `bin/node` — verified). See
> [`private-cutover-integration-gap.md`](private-cutover-integration-gap.md).
>
> **This runbook does not yet yield a working zero-exposure tunnel.** It stands
> up the real coordinator and shows the invite/entry path that works today, then
> marks precisely where the node must learn to discover its reflexive,
> hole-punch, bring up WireGuard, and relay. The CI simulated-NAT suite (Slice 3,
> `crates/system/nat-traversal/tests/simnat_ci.rs`) proves the *logic*; this
> runbook is what turns it into a *deployment* once the node is wired.

## Topology

Three hosts:

- **Coordinator** — a public VPS, `p2p.ducktape.industries`, deployed per
  [`coordinator.md`](coordinator.md). Untrusted; no key; UDP `:3478`.
- **Validator A** — behind its own NAT, no inbound port-forward.
- **Validator B** — behind a *different* NAT, no inbound port-forward.

```
   Validator A ──┐                          ┌── Validator B
   (NAT, no      │      Coordinator         │   (NAT, no
    inbound)     └────▶ p2p.ducktape ◀──────┘    inbound)
                        (public VPS)
     A and B dial OUT to the coordinator; ideally they then hole-punch a
     direct A<->B WireGuard tunnel and drop the coordinator out of the path.
```

This extends the Ducktape-2 live-join rig (v2/v3 invite format, the NAT-hairpin
gotcha, the 2-validator-quorum teardown caveat).

## Prerequisites

- The coordinator deployed and answering (per [`coordinator.md`](coordinator.md);
  verify with the Task-2 subprocess proof or `ss -lunp 'sport = :3478'`).
- Validator A and Validator B each have a built `ducktape-node`.
- A founder able to mint v3 invites.
- For the (future) tunnel steps: real WireGuard userspace/kernel on A and B.

## The tagged procedure

1. **Deploy the coordinator on the VPS.** `[WORKS TODAY]` — follow [`coordinator.md`](coordinator.md); confirm it binds UDP `:3478` (`ss -lunp 'sport = :3478'`) and answers a live `BindRequest` (the `deploy_smoke.rs` subprocess proof exercises exactly this).
2. **Mint a v3 invite carrying a `Coordinated` reach hint.** `[WORKS TODAY]` (encoding only) — Slice 1's `INVITE_PREFIX_V3` + `ReachHint`/`CoordRef` produce `coordinated:<ek>@p2p.ducktape.industries:3478#<coord_key>`, and it round-trips through `bin/node/src/config.rs` `pack`/`unpack`/`parse`. **Partial:** the invite *encodes* the coordinator correctly, but the node does not yet *consume* the hint as a reachability path — see step 5.
3. **A and B each generate an identity and get admitted.** `[WORKS TODAY]` — the founder runs `invite-accept`; this is the unchanged admission path, independent of reachability.
4. **A and B boot dial-out-only against the coordinator's reflexive/rendezvous service.** `[NEEDS NODE WIRING]` — the node never constructs a `NatClient`, never sends a `BindRequest`, never `register`s. The coordinator *would* answer (Task 2 proves it), but nothing in `bin/node` asks. Mechanism exists, unwired: `nat_traversal::NatClient::{bind_multi, discover_reflexive_failover, register}`.
5. **A `Coordinated` hint is consumed as a reachability path.** `[NEEDS NODE WIRING]` — verified stub: `reach_entries()` (config.rs ~337-354) feeds `coord_addr` into the commonware **TCP** `bootstrappers`, so today the node would try to open a *mesh* connection to the coordinator's **UDP** port and fail. The hint must instead be routed to a `NatClient`. (This is why step 2 is only *partial*.)
6. **A and B publish their reflexive endpoints and rendezvous.** `[NEEDS NODE WIRING]` — no reflexive is discovered or published into `EndpointAdvertisementV1.wireguard_endpoint`; no `lookup`/`recv_punch_sync`. Mechanism exists (`nat-traversal` rendezvous + the `wireguard-upgrade` advertisement + Slice 3's `AdvertBook`), unwired.
7. **A and B hole-punch a direct WireGuard tunnel (coordinator-timed simultaneous open).** `[NEEDS NODE WIRING]` — `send_punch_to`/`recv_punch_from` exist and are CI-proven (`drive_simulated`), but the node never drives them, and no WireGuard interface is created (`wireguard_effect::apply_tunnel_plan` / `DefguardWireGuardEffect` is never called — the crate's own module doc says nothing in the workspace calls `WGApi::configure_interface` today).
8. **On hole-punch failure, fall back to the coordinator ciphertext relay.** `[NEEDS NODE WIRING]` — `request_relay` + `peer_endpoint_override` exist and are CI-proven (`drive_with_relay_fallback`), unwired; **and** the relay-bind caveat applies (the coordinator must bind its routable public IP, not `0.0.0.0`, or relay grants are undialable — see [`coordinator.md`](coordinator.md)).
9. **Real state-sync / app-hash flows over the tunnel.** `[NEEDS NODE WIRING]` — depends on steps 6-8; there is nothing to run today.

## What you CAN demo today vs. what proves the tunnel

**Today, with shipped binaries, you can:**

- Deploy the coordinator and prove it answers (the `deploy_smoke.rs` subprocess
  proof; step 1).
- Mint and parse a v3 `Coordinated` invite (step 2, encoding).
- Admit A and B (step 3).

**You cannot yet** do any step tagged `[NEEDS NODE WIRING]` (steps 4-9): the node
constructs no `NatClient`, discovers no reflexive, punches nothing, brings up no
WireGuard interface, and relays nothing. The logic behind those steps is already
proven at the library level by the CI simulated-NAT suite
(`crates/system/nat-traversal/tests/simnat_ci.rs`, Slice 3) with a fake NAT and a
`FakeWireGuardEffect`. The moment the node wiring lands (the
[integration-gap handoff](private-cutover-integration-gap.md) §3 checklist, the
"Slice 5" follow-on), this runbook becomes the Acceptance §2 procedure verbatim —
the `[NEEDS NODE WIRING]` tags flip to `[WORKS TODAY]` step by step, and the real
cross-machine run additionally needs the user's infra (two real NATs, a VPS with
the relay-bind caveat handled, real WireGuard on A and B).
