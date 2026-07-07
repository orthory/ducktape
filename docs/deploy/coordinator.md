# Coordinator Deployment Recipe (`p2p.ducktape.industries`)

How to run `bin/coordinator` as `p2p.ducktape.industries`: an **untrusted,
non-validator reachability helper** that lets two NAT'd validators find each
other and hole-punch a direct path. It is rendezvous-only: it never carries
peer traffic (the DERP-style ciphertext relay was removed 2026-07-06). This is
the operator-facing companion to the design of record,
[Private Cutover — Coordinator](../superpowers/specs/2026-07-05-private-cutover-coordinator-design.md).
Read that for the *why* (the trust model, the reachability plane, the epic
roadmap). This document is the *how*.

> **Scope honesty.** Everything on this page **works today**: `bin/coordinator`
> runs as-is and its `--listen` invocation is regression-proven by
> `bin/coordinator/tests/deploy_smoke.rs`. On the other end, the node-side
> reachability plane is wired but **staged**: `bin/node` constructs a
> `reachability::NatResolver` (reflexive discovery, `register`, hole-punch
> against the configured coordinators) only when `wireguard_listen` is
> configured. What still does not work end-to-end (v3 `Coordinated` hint
> consumption, coordinator-auth) is tracked in
> [the integration-gap handoff](private-cutover-integration-gap.md); the
> cross-machine procedure is
> [the runbook](cross-machine-zero-exposure-runbook.md).

## Why this is safe (untrusted by design)

The two planes this coordinator sits beside are both authenticated and
end-to-end encrypted **without** the coordinator:

- The **control mesh** is commonware `authenticated::discovery`: a dialer dials
  an address and expects a specific `ed25519` public key; the handshake
  authenticates that key regardless of what network path delivered the bytes
  (same property that makes the Phase-1 [sentry](../sentry-deployment.md) safe).
- The **data tunnel** is validator↔validator **WireGuard**, keyed and encrypted
  between the two validators' own keys.

So the coordinator is at most a rendezvous point beside those planes — it is
never *on* a data path at all. It **learns coarse topology and reflexive
addresses and can observe rendezvous timing, but it cannot decrypt,
impersonate, MITM, serve state, or join consensus** (design §"Trust and threat
model"). Therefore it is safe to run as **throwaway infra with no key on the
box**. The hardening in `ops/coordinator/ducktape-coordinator.service` makes that
structural: if a reviewer can find a place a secret *would* live on this host,
the recipe is wrong.

## What it is

`coordinator --listen <addr>` (default `0.0.0.0:3478`). One **UDP** *control*
socket — genuinely the only socket the process ever holds. Stateless. It
provides two services on that one socket:

- **Rendezvous** — peers `register` their key and `lookup` each other; a
  `Lookup` fans a `PunchSync` to both sides so they simultaneous-open.
- **STUN reflexive** — answers a `BindRequest` with the peer's observed
  public `ip:port` so it can learn its own NAT-mapped address.

Registrations have a lifetime: a `register`/`readvertise` mapping expires
`REGISTRATION_TTL_SECS` (120 s) after the last accepted advert; an expired key
resolves to `None` and receives no `PunchSync` (its NAT pinhole is long dead
anyway). Live nodes hold their mapping with a 25 s keepalive `Readvertise`
(`reachability::RENDEZVOUS_KEEPALIVE`), which doubles as the NAT-pinhole
keepalive. The book heals itself across coordinator restarts — the same
keepalives re-register everyone within one interval (their nonces are
wall-clock-seeded, so a rebooted node supersedes its own stale mapping instead
of being rejected as a replay).

Everything it answers derives from the **observed source** of the datagram, so
a wildcard `--listen 0.0.0.0:3478` bind is fully functional on a single-IP
host. **Multi-homed caveat:** on a box with more than one routable IP, bind the
concrete public IP peers dial. Replies from a wildcard-bound UDP socket egress
with the kernel's route-chosen source address, and `NatClient` (correctly)
discards any reply that does not come from the exact address it dialed — so a
coordinator answering from the "wrong" IP looks healthy while every client
times out.

No TCP listener. No disk. No secret. On bind it prints
`coordinator listening on <addr>` to stderr, then serves.

## Auth modes

The coordinator is keyless in every mode:

- **Default public mode** — no auth flag. Requests must carry proof of
  possession for the node key they claim.
- **Private mode** — `--genesis-set <network.toml>`. The coordinator reads only
  the public `validators = [...]` keys from that descriptor and admits genesis
  validators or holder-presented caps rooted in that set.
- **Legacy development mode** — `--allow-anonymous`. This disables proof of
  possession and is for local smoke testing only.

Malformed `--listen` and malformed/value-less `--genesis-set` are hard errors,
not silent fallbacks to a weaker policy.

## Deploy A — systemd (bare VPS)

```sh
# 1. Build the binary (repo root).
cargo build --release -p coordinator-bin

# 2. Install it.
sudo install -m 0755 target/release/coordinator /usr/local/bin/ducktape-coordinator

# 3. Install the env file and the hardened unit.
sudo install -D -m 0644 ops/coordinator/coordinator.env.example /etc/ducktape/coordinator.env
sudo cp ops/coordinator/ducktape-coordinator.service /etc/systemd/system/

# Optional: edit /etc/ducktape/coordinator.env to choose a bind address and auth
# mode. Leave COORDINATOR_ARGS empty for default public proof-of-possession, or
# use: COORDINATOR_ARGS=--genesis-set /etc/ducktape/network.toml

# 4. Start it.
sudo systemctl daemon-reload
sudo systemctl enable --now ducktape-coordinator

# 5. Verify.
systemctl status ducktape-coordinator
ss -lunp 'sport = :3478'     # the fixed control socket, owned by the dynamic user
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

# 3478 is the only socket, so one published UDP port is enough. Harden the
# container to match the systemd unit — this is untrusted-by-design infra, so
# drop everything it does not need.
docker run \
  --cap-drop=ALL \
  --security-opt no-new-privileges \
  --read-only \
  --restart unless-stopped \
  -p 3478:3478/udp \
  ducktape-coordinator
```

For private mode in Docker, append the auth args after the image name:

```sh
docker run --cap-drop=ALL --security-opt no-new-privileges --read-only \
  -p 3478:3478/udp \
  -v /etc/ducktape/network.toml:/etc/ducktape/network.toml:ro \
  ducktape-coordinator --listen 0.0.0.0:3478 --genesis-set /etc/ducktape/network.toml
```

The image is multi-stage: a `rust:1.96-bookworm` build stage compiles exactly
`-p coordinator-bin`, and the runtime stage is
`gcr.io/distroless/cc-debian12:nonroot` — glibc + libgcc for the
dynamically-linked binary, **no shell, no package manager**, running as the
built-in non-root uid `65532`. The `docker run` flags above are not optional
decoration — they mirror the systemd unit's posture and are the container-side
equivalent of its empty capability set and read-only root:

- `--cap-drop=ALL` — the binary needs **no** Linux capabilities (3478 > 1024, so
  no privileged-port capability), the same as the unit's empty
  `CapabilityBoundingSet=`/`AmbientCapabilities=`.
- `--security-opt no-new-privileges` — mirrors `NoNewPrivileges=yes`.
- `--read-only` — the coordinator keeps **no** state, so its root filesystem can
  be immutable, mirroring `ProtectSystem=strict` + `ReadOnlyPaths=/`.
- `--restart unless-stopped` — the production restart policy; the disposable box
  comes back after a crash or reboot. Drop it (and add `--rm`) only for a
  throwaway smoke run.

Keep these: the whole premise is that a compromised coordinator has nothing to
steal and nowhere to write. Publishing **only** `-p 3478:3478/udp` (not
`--network host`) is what keeps that true — with rendezvous-only there is no
ephemeral-socket caveat, so bridge networking covers everything the process
does.

## DNS + firewall

- Point an `A` record `p2p.ducktape.industries` → the VPS IP.
- Open **inbound UDP 3478**. No TCP port is needed at all, and no other UDP
  port either — the coordinator never binds a second socket.

## Redundancy — the coordinator is not load-bearing

Run **multiple** coordinators. A v3 invite carries a `Vec` of reach hints and
`NatClient::discover_reflexive_failover` walks them (Slice 3), so a single
coordinator outage is not fatal to entry. And an already-**punched** direct path
survives a coordinator restart entirely — only *new* rendezvous depends on a
live coordinator; no data path ever traverses one. This is the key contrast
with an in-path [sentry](../sentry-deployment.md), which sits in the data path
and is a single point of failure for the validator it fronts: an out-of-path
coordinator is where the "established connections survive; only new ones depend
on it" framing actually holds.

## What this recipe does NOT do (forward reference)

The coordinator deployed here is live and correct, and the node-side
reachability plane (staged behind `wireguard_listen`) drives it: `bin/node`
constructs a `NatResolver` that discovers its reflexive, `register`s, and
hole-punches against the configured coordinators. What this page still does
**not** claim is a demonstrated end-to-end zero-exposure tunnel: a v3
`Coordinated` invite hint is not yet consumed as a reachability path, and
coordinator-auth remains open. For the exact works-today line, see
[`private-cutover-integration-gap.md`](private-cutover-integration-gap.md);
the cross-machine procedure is
[`cross-machine-zero-exposure-runbook.md`](cross-machine-zero-exposure-runbook.md).
