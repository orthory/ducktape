# Coordinator Deployment Recipe (`p2p.ducktape.byeongsu.dev`)

How to run `bin/coordinator` as `p2p.ducktape.byeongsu.dev`: an **untrusted,
non-validator reachability helper** that lets two NAT'd validators find each
other and hole-punch a direct path. Its optional TCP lane carries only sealed
first-contact admission datagrams when UDP setup is exhausted; it never carries
the established overlay or peer data. This runbook, the coordinator code, and
the regression tests are the maintained source of record for the trust model
and operating contract.

> **Scope honesty.** Everything on this page **works today**: `bin/coordinator`
> runs as-is and its `--listen` invocation is regression-proven by
> `bin/coordinator/tests/deploy_smoke.rs`. On the other end, the node-side
> reachability plane is wired behind `wireguard_listen`: `bin/node` constructs a
> `reachability::NatResolver` (reflexive discovery, `register`, hole-punch
> against the configured coordinators) only when `wireguard_listen` is
> configured. v1 `Coordinated` hints are consumed as reachability routes;
> successful punching is still NAT-dependent because there is no relay fallback.
> The cross-machine procedure is [the runbook](cross-machine-zero-exposure-runbook.md).

## Why this is safe (untrusted by design)

The two planes this coordinator sits beside are both authenticated and
end-to-end encrypted **without** the coordinator:

- The **control mesh** is commonware `authenticated::discovery`: a dialer dials
  an address and expects a specific `ed25519` public key; the handshake
  authenticates that key regardless of what network path delivered the bytes
  (same property that makes the Phase-1 [sentry](sentry-deployment.md) safe).
- The **data tunnel** is validator↔validator **WireGuard**, keyed and encrypted
  between the two validators' own keys.

So the coordinator sits beside those planes as a rendezvous point. What a
compromised or malicious coordinator **cannot** do is the load-bearing
guarantee: it **holds no key, and cannot decrypt, impersonate, forge, serve
state, or join consensus** — WireGuard's end-to-end encryption and the
`authenticated::discovery` dial-expects-key handshake hold regardless of what
path delivers the bytes. Therefore it is safe to run as **throwaway infra with
no key on the box**. The hardening in
`ops/coordinator/ducktape-coordinator.service` makes that structural: if a
reviewer can find a place a secret *would* live on this host, the recipe is
wrong.

> **What it CAN do — read this before trusting a third party's coordinator.**
> The coordinator is untrusted for *confidentiality and authenticity*, but it is
> **not** out of the data path in every case. It brokers the punched endpoint
> each side installs, and the underlay endpoint is **not** cryptographically
> bound to the peer's identity — only the peer's WireGuard *public key* is
> pinned. So a malicious coordinator can answer a lookup with **its own**
> address, punch from it, and become the **underlay relay** between two peers
> that have no direct path. Riding that relay it can **observe traffic volume
> and timing** and **censor** (drop) an established tunnel — but it still cannot
> read, forge, or alter the WireGuard-encrypted payload. This is **inherent** to
> rendezvous-based NAT traversal: for a peer pair with no direct route, the
> party that tells each side where the other *is* must be trusted not to name
> itself. A signed punch would not close it (the coordinator can relay the
> peer's own valid packets); only a directly-reachable signed endpoint lets a
> peer bypass the coordinator entirely.
>
> **Operational guidance:** run coordinators **you** trust (or your own), and run
> **several** — a peer that can reach a direct signed endpoint never installs a
> coordinator-punched override, and multiple coordinators dilute any single
> one's leverage. Do not treat a stranger's coordinator as neutral: it cannot
> steal your keys or your data, but it can watch and throttle the connections
> that depend on it.

## What it is

`coordinator --listen <addr> --relay-listen <addr|none> --workers <1|4>`
(defaults: UDP `0.0.0.0:3478`, TCP `0.0.0.0:443`, one worker). It is stateless.
The UDP control socket provides two services:

- **Rendezvous** — peers `register` their key and `lookup` each other; a
  `Lookup` fans a `PunchSync` to both sides so they simultaneous-open.
- **STUN reflexive** — answers a `BindRequest` with the peer's observed
  public `ip:port` so it can learn its own NAT-mapped address.

The optional TCP listener is a bounded fallback for the sealed first-contact
intro only. It resolves a member through the same live advert book, forwards
one opaque datagram, and returns the member's opaque reply. It is not a
WireGuard-over-TCP or general peer-data relay. Pass `--relay-listen none` when
that admission fallback is not deployed.

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

No disk. No secret. On bind it prints `coordinator listening on <addr>` and,
when enabled, `coordinator relay listening on tcp/<addr>` to stderr, then
serves.

## Auth modes

The coordinator is keyless in every mode:

- **Default public mode** — no auth flag. Requests must carry proof of
  possession for the node key they claim.
- **Private mode** — `--genesis-set <network.toml>`. The coordinator reads only
  the public `validators = [...]` keys from that descriptor and admits genesis
  validators or holder-presented caps rooted in that set.

Malformed `--listen`, `--relay-listen`, `--workers`, `--metrics-interval`, and
malformed/value-less `--genesis-set` are hard errors, not silent fallbacks to a
weaker policy.

## Authentication workers

`--workers 1` verifies inline with no worker threads. `--workers 4` runs only
the Ed25519 checks on four fixed 512 KiB-stack threads; UDP I/O and the single
rendezvous state machine remain ordered on the current-thread runtime. The
workers pull from one shared bounded queue, so an idle verifier can take the
next job instead of waiting behind a busy worker's round-robin queue. The work
queue, result queue, and ordered completion window are bounded, so overload
falls back to the kernel's bounded UDP queue/drop behavior instead of growing
process memory.

The systemd and container recipes select `--workers 4` and set
`MALLOC_ARENA_MAX=1` to avoid glibc reserving a large virtual arena per worker.
Use `--workers 1` on a single-vCPU or minimum-footprint host.

## Metrics, cross-host load, flood, and soak

The server emits one parseable `coordinator_metrics` line every
`--metrics-interval` seconds (default 10; `0` disables it). Counters are
cumulative. `cpu_pct` is process CPU across all coordinator threads, so four
fully busy auth workers can approach 400%. `rss_mib` and the
`inflight`/`inflight_max`/`saturated` fields make bounded overload and recovery
visible without opening a metrics socket.

Build both executables, put `coordinator` on the server host and
`coordinator-load` on a different host, then run the baseline:

```sh
cargo build --release -p coordinator-bin --bins

# Server host. With systemd, set the same arguments in COORDINATOR_ARGS and use
# journalctl -fu ducktape-coordinator for the metrics rows.
MALLOC_ARENA_MAX=1 ./coordinator --listen 0.0.0.0:3478 --workers 4 \
  --metrics-interval 1

# Load host (a distinct machine/VM): client-local RTT needs no clock sync.
# Sweep concurrency until coordinator CPU or drop rate reaches the desired load.
for clients in 16 64 256; do
  ./coordinator-load --target SERVER_IP:3478 --duration 60 \
    --clients "$clients" --report-interval 0
done
```

The default output is a compact comparison table with end-to-end p99, drops,
request/flood rates, and load-host CPU; use `--output log` for stable
`key=value` ingestion. The matching grouped server metrics rows supply
coordinator CPU/RSS and show whether the 260-request bounded window saturated.
Each valid client reuses one signed, response-correlated request, refreshes its
timestamp every 20 seconds, and rotates the correlation key after a timeout.
That keeps late replies out of later latency samples without making load-host
signing throughput the benchmark's ceiling.
Latency uses fixed 0.1 ms buckets up to the configured timeout, so a 24-hour
run keeps constant memory instead of retaining every sample.

Run a real invalid-signature flood while retaining a valid probe, followed by a
valid-only recovery phase:

```sh
./coordinator-load --target SERVER_IP:3478 --duration 60 --clients 1 \
  --invalid-clients 16 --timeout-ms 1000 --recovery 10 --report-interval 10
```

Invalid packets use a fresh claimed public key but a different signing key and
refresh their timestamp every 10 seconds, so they exercise Ed25519 rejection
rather than the cheaper stale-request check. Recovery passes only if at least
one valid response arrives after the flood stops; on the server,
`rejected` rises during the flood and `inflight` returns to zero afterward.

The same bounded-memory probe is the 24-hour soak; interval rows keep the run
observable without retaining every latency sample:

```sh
./coordinator-load --target SERVER_IP:3478 --duration 86400 --clients 16 \
  --rate 1000 --timeout-ms 1000 --report-interval 60 | tee coordinator-soak.log
```

Keep the corresponding server metrics log. A useful soak artifact therefore
contains minute p99/drop rows plus CPU, RSS, saturation, send-error, and recovery
evidence from the server. Omit `--rate` only when the soak host can sustain an
unlimited run without disturbing colocated services. Do not add `NodeKey`
sharding unless this cross-host profile shows the ordered state actor—not
signature verification or UDP drops—has become the measured bottleneck.

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
# mode. The supplied file selects four auth workers, public proof-of-possession,
# and no TCP fallback. For private mode use:
# COORDINATOR_ARGS=--relay-listen none --workers 4 --metrics-interval 10 --genesis-set /etc/ducktape/network.toml

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
`RestrictAddressFamilies=AF_INET AF_INET6` (Internet IP sockets only; no
unix/raw/packet sockets). This posture is deliberate: **there is no secret to
steal and no state to corrupt**, so the box can be treated as disposable and
replaced at will.

## Deploy B — Docker / OCI

```sh
docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .

# This deployment explicitly disables the TCP fallback, so one published UDP
# port is enough. Harden the container to match the systemd unit.
docker run \
  --cap-drop=ALL \
  --security-opt no-new-privileges \
  --read-only \
  --restart unless-stopped \
  -p 3478:3478/udp \
  ducktape-coordinator --listen 0.0.0.0:3478 --relay-listen none --workers 4
```

For private mode in Docker, append the auth args after the image name:

```sh
docker run --cap-drop=ALL --security-opt no-new-privileges --read-only \
  -p 3478:3478/udp \
  -v /etc/ducktape/network.toml:/etc/ducktape/network.toml:ro \
  ducktape-coordinator --listen 0.0.0.0:3478 --relay-listen none --workers 4 \
    --genesis-set /etc/ducktape/network.toml
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
`--network host`) is sufficient for the explicit `--relay-listen none`
deployment above. To enable the sealed-intro fallback, add
`-p 443:8443/tcp` and `--relay-listen 0.0.0.0:8443`; the host mapping exposes
the standard port without granting the container a privileged bind.

## DNS + firewall

- Point an `A` record `p2p.ducktape.byeongsu.dev` → the VPS IP.
- Open **inbound UDP 3478**. When the sealed-intro fallback is enabled, also
  open its externally mapped TCP port (normally 443).

## Redundancy — the coordinator is not load-bearing

Run **multiple** coordinators. A v1 invite carries a `Vec` of reach hints and
`NatClient::discover_reflexive_failover` walks them, so a single
coordinator outage is not fatal to entry. A tunnel punched to a peer's **own**
address (a direct signed endpoint, or a reflexive that stayed valid) survives a
coordinator restart entirely — only *new* rendezvous depends on a live
coordinator. The one caveat is the relay case above: if a **malicious**
coordinator made itself a peer's underlay endpoint, that tunnel rides *it*, so
that tunnel does depend on it (and it can drop it) — which is exactly why you
run coordinators you trust and prefer direct signed endpoints. This is still a
sharp contrast with an in-path [sentry](sentry-deployment.md), which is
*always* in the data path and a single point of failure for the validator it
fronts: an honest out-of-path coordinator is where the "established connections
survive; only new ones depend on it" framing holds.

## What this recipe does NOT do

The coordinator deployed here is live and correct, and the node-side
reachability plane (behind `wireguard_listen`) drives it: `bin/node`
constructs a `NatResolver` that discovers its reflexive, `register`s, and
hole-punches against the configured coordinators, and v1 `Coordinated` invite
hints route into that path instead of being dialed as mesh peers.

What this page does **not** promise is universal zero-exposure connectivity:
the coordinator is rendezvous-only, so NAT pairs that cannot punch fail
honestly rather than falling back to a relay. The cross-machine procedure is
[`cross-machine-zero-exposure-runbook.md`](cross-machine-zero-exposure-runbook.md).
