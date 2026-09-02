# `ops/coordinator/` — deploy artifacts for the untrusted coordinator

Three ready-to-use artifacts for running `bin/coordinator` as
`relay.ducktape.industries`. The coordinator is the private-cutover reachability
helper: UDP STUN/rendezvous plus an optional TCP fallback for the sealed
first-contact intro. It never carries the established overlay or peer data. It
**holds no keys, serves no state, and is untrusted by design** — see
`docs/deploy/coordinator.md` for the maintained operator recipe and trust-model
notes.

- **`ducktape-coordinator.service`** — hardened systemd unit (DynamicUser,
  `CAP_NET_BIND_SERVICE` only — the relay lane binds 443 — read-only
  filesystem, Internet IP sockets only).
- **`coordinator.env.example`** — the single operator-edited line, the bind
  address, plus optional auth-mode args. **Not a secret**; the coordinator has
  none.
- **`Dockerfile`** — multi-stage, distroless `cc-debian12:nonroot` runtime.

## systemd (bare VPS)

```sh
cargo build --release -p coordinator-bin
sudo install -m 0755 target/release/coordinator /usr/local/bin/ducktape-coordinator
sudo install -D -m 0644 ops/coordinator/coordinator.env.example /etc/ducktape/coordinator.env  # edit bind/auth mode
sudo cp ops/coordinator/ducktape-coordinator.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now ducktape-coordinator
```

## Docker / OCI

The relay lane binds an unprivileged port inside the container and the host
maps TCP 443 onto it, so the non-root container needs no privileged bind:

```sh
docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
docker run --rm -p 3478:3478/udp -p 443:8443/tcp ducktape-coordinator \
  --listen 0.0.0.0:3478 --relay-listen 0.0.0.0:8443
```

Auth modes:

- default: public proof-of-possession, relay lane on
  (`COORDINATOR_ARGS=--relay-listen 0.0.0.0:443 --workers 4 --metrics-interval 10`).
- private: append `--genesis-set /etc/ducktape/network.toml`.

Keep the relay on: every joiner derives its first-contact fallback as
`<coordinator host>:443` and is never told otherwise. A failed 443 bind does
not stop the coordinator — it prints `ERROR: relay lane DISABLED` at boot and
every `coordinator_metrics` row carries `relay=off`.

`coordinator_metrics` lines report request counters, bounded-window saturation,
in-flight work, `relay=on|off` plus relay session counters, process CPU, and
RSS. The cross-host/flood/24-hour probe commands are in
`docs/deploy/coordinator.md`.

A `--listen 0.0.0.0:3478` wildcard bind is fully functional on a single-IP
host: every answer derives from the datagram's observed source. On a
**multi-homed** host, bind the concrete public IP peers dial — replies from a
wildcard socket egress with the route-chosen source IP, and clients discard
replies that don't come from the address they dialed. Full detail in
`docs/deploy/coordinator.md`.
