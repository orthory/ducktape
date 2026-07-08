# `ops/coordinator/` — deploy artifacts for the untrusted coordinator

Three ready-to-use artifacts for running `bin/coordinator` as
`p2p.ducktape.byeongsu.dev`. The coordinator is the private-cutover reachability
helper: STUN reflexive + rendezvous over a single UDP socket — rendezvous
only, it never carries peer traffic. It **holds no keys, serves no state, and
is untrusted by design** — see `docs/deploy/coordinator.md` for the maintained
operator recipe and trust-model notes.

- **`ducktape-coordinator.service`** — hardened systemd unit (DynamicUser,
  empty capability set, read-only filesystem, UDP-only address families).
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

```sh
docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
docker run --rm -p 3478:3478/udp ducktape-coordinator
```

Auth modes:

- default: public proof-of-possession (`COORDINATOR_ARGS=`).
- private: `COORDINATOR_ARGS=--genesis-set /etc/ducktape/network.toml`.
- local/dev legacy: `COORDINATOR_ARGS=--allow-anonymous`.

A `--listen 0.0.0.0:3478` wildcard bind is fully functional on a single-IP
host: every answer derives from the datagram's observed source. On a
**multi-homed** host, bind the concrete public IP peers dial — replies from a
wildcard socket egress with the route-chosen source IP, and clients discard
replies that don't come from the address they dialed. Full detail in
`docs/deploy/coordinator.md`.
