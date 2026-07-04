# Coordinator Deployment Recipe (`p2p.ducktape.industries`)

How to run `bin/coordinator` as `p2p.ducktape.industries`: an **untrusted,
non-validator reachability helper** that lets two NAT'd validators find each
other and, when a direct path can't be punched, relays their ciphertext. This is
the operator-facing companion to the design of record,
[Private Cutover — Coordinator](superpowers/specs/2026-07-05-private-cutover-coordinator-design.md).
Read that for the *why* (the trust model, the reachability plane, the epic
roadmap). This document is the *how*.

> **Scope honesty.** Everything on this page **works today**: `bin/coordinator`
> runs as-is and its `--listen` invocation is regression-proven by
> `bin/coordinator/tests/deploy_smoke.rs`. What does **not** work yet is the
> other end: the live `ducktape-node` does not call the coordinator (reflexive
> discovery, hole-punch, WireGuard bring-up, and relay are unwired in the node —
> `nat-traversal` and `wireguard-effect` are not dependencies of `bin/node`).
> Deploying this coordinator is real and useful, but it does **not** by itself
> yield a working zero-exposure tunnel. See
> [the cross-machine runbook](cross-machine-zero-exposure-runbook.md) (every
> step tagged) and [the integration-gap handoff](private-cutover-integration-gap.md).

## Why this is safe (untrusted by design)

The two planes this coordinator sits beside are both authenticated and
end-to-end encrypted **without** the coordinator:

- The **control mesh** is commonware `authenticated::discovery`: a dialer dials
  an address and expects a specific `ed25519` public key; the handshake
  authenticates that key regardless of what network path delivered the bytes
  (same property that makes the Phase-1 [sentry](../sentry-deployment.md) safe).
- The **data tunnel** is validator↔validator **WireGuard**, keyed and encrypted
  between the two validators' own keys.

So any box on the path — including this coordinator — is at most a transparent
ciphertext forwarder plus a rendezvous point. It **learns coarse topology and
reflexive addresses and can observe ciphertext + timing, but it cannot decrypt,
impersonate, MITM, serve state, or join consensus** (design §"Trust and threat
model"). Therefore it is safe to run as **throwaway infra with no key on the
box**. The hardening in `ops/coordinator/ducktape-coordinator.service` makes that
structural: if a reviewer can find a place a secret *would* live on this host,
the recipe is wrong.

## What it is

`coordinator --listen <addr>` (default `0.0.0.0:3478`). One **UDP** socket.
Stateless. It provides three services on that one socket:

- **Rendezvous** — peers `register` their key and `lookup` each other.
- **STUN reflexive** — answers a `BindRequest` with the peer's observed
  public `ip:port` so it can learn its own NAT-mapped address.
- **Ciphertext relay** — a last-resort splice when hole-punch fails; the
  coordinator forwards opaque bytes between two peers' relay sockets.

No TCP listener. No config file. No disk. No secret. `--listen` is the only flag
the binary parses; on bind it prints `coordinator listening on <addr>` to
stderr, then serves.

## Deploy A — systemd (bare VPS)

```sh
# 1. Build the binary (repo root).
cargo build --release -p coordinator-bin

# 2. Install it.
sudo install -m 0755 target/release/coordinator /usr/local/bin/ducktape-coordinator

# 3. Install the env file and the hardened unit.
sudo install -D -m 0644 ops/coordinator/coordinator.env.example /etc/ducktape/coordinator.env
#    edit /etc/ducktape/coordinator.env if you need the relay bind (see caveat)
sudo cp ops/coordinator/ducktape-coordinator.service /etc/systemd/system/

# 4. Start it.
sudo systemctl daemon-reload
sudo systemctl enable --now ducktape-coordinator

# 5. Verify.
systemctl status ducktape-coordinator
ss -lunp 'sport = :3478'     # one UDP socket, owned by the dynamic user
```

The unit runs under `DynamicUser=yes` (an ephemeral throwaway user — no home,
no shell, nothing to compromise), with `CapabilityBoundingSet=` and
`AmbientCapabilities=` **empty** (port 3478 > 1024 needs no privileged-port
capability), `NoNewPrivileges=yes`, `ProtectSystem=strict` + `ReadOnlyPaths=/`
(no writable path at all — the coordinator keeps no state), and
`RestrictAddressFamilies=AF_INET AF_INET6` (UDP only; no unix/raw/packet
sockets). This posture is deliberate: **there is no secret to steal and no state
to corrupt**, so the box can be treated as disposable and replaced at will.

## Deploy B — Docker / OCI

```sh
docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
docker run --rm -p 3478:3478/udp ducktape-coordinator
```

The image is multi-stage: a `rust:1.96-bookworm` build stage compiles exactly
`-p coordinator-bin`, and the runtime stage is
`gcr.io/distroless/cc-debian12:nonroot` — glibc + libgcc for the
dynamically-linked binary, **no shell, no package manager**, running as the
built-in non-root uid `65532`. For the **relay** fallback, publishing a mapped
port is not enough (see the caveat below): use `--network host` and
`--listen <public-ip>:3478` so relay grants inherit a dialable IP.

## The relay-bind caveat (load-bearing)

`nat_traversal::run_coordinator` binds its relay-splice sockets on the
coordinator socket's **own IP** (`bind_ip = sock.local_addr().ip()`). So if you
launch with `--listen 0.0.0.0:3478`, a `RelayGrant` hands the client
`0.0.0.0:<ephemeral>` — **not remotely dialable**. STUN reflexive, rendezvous
`Lookup`, and hole-punch `PunchSync` are all **unaffected** (they echo/return the
peer's *observed source*, independent of the coordinator's bind IP); **only the
ciphertext-relay fallback** breaks under a wildcard bind.

**If you need relay fallback, bind the routable public IP** —
`--listen 203.0.113.10:3478` — so relay grants are dialable. A future
coordinator-side fix (learn the public reflexive from observed peer sources and
emit *that* as the relay-grant IP) is described in
[the integration-gap doc](private-cutover-integration-gap.md) §4.

## DNS + firewall

- Point an `A` record `p2p.ducktape.industries` → the VPS IP.
- Open **inbound UDP 3478**. No TCP port is needed at all.
- For **relay**, either bind the public IP (recommended — then grants use 3478's
  IP) or open the ephemeral UDP range the OS assigns to relay-splice sockets.

## Redundancy — the coordinator is not uniquely load-bearing

Run **multiple** coordinators. A v3 invite carries a `Vec` of reach hints and
`NatClient::discover_reflexive_failover` walks them (Slice 3), so a single
coordinator outage is not fatal to entry. And an already-**punched** direct path
survives a coordinator restart entirely — only *relay* fallback and *new*
rendezvous depend on a live coordinator. This is the key contrast with an
in-path [sentry](../sentry-deployment.md), which sits in the data path and is a
single point of failure for the validator it fronts: an out-of-path coordinator
is where the "established connections survive; only new ones depend on it"
framing actually holds.

## What this recipe does NOT do (forward reference)

The coordinator deployed here is live and correct, **but the `ducktape-node`
does not yet use it.** Reflexive discovery, hole-punch, WireGuard bring-up, and
relay are all **unwired in the node** — the mechanism is CI-proven in
`crates/system/nat-traversal` and `crates/system/wireguard-effect`, but neither
crate is a dependency of `bin/node`, and a v3 `Coordinated` invite hint is
currently dialed as an ordinary **TCP mesh bootstrapper** at the coordinator's
**UDP** address (a no-op-at-best). Do **not** read this page as claiming a
zero-exposure tunnel. For the exact works-today / needs-wiring line, see
[`cross-machine-zero-exposure-runbook.md`](cross-machine-zero-exposure-runbook.md);
for the engineering handoff to close the gap, see
[`private-cutover-integration-gap.md`](private-cutover-integration-gap.md).
