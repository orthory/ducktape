# `ops/coordinator/` — deploy artifacts for the untrusted coordinator

Three ready-to-use artifacts for running `bin/coordinator` as
`p2p.ducktape.industries`. The coordinator is the private-cutover reachability
helper: STUN reflexive + rendezvous + ciphertext relay over a single UDP
socket. It **holds no keys, serves no state, and is untrusted by design** — see
`docs/deploy/coordinator.md` for the full recipe and the design of record at
`docs/superpowers/specs/2026-07-05-private-cutover-coordinator-design.md`.

- **`ducktape-coordinator.service`** — hardened systemd unit (DynamicUser,
  empty capability set, read-only filesystem, UDP-only address families).
- **`coordinator.env.example`** — the single operator-edited line, the bind
  address. **Not a secret**; the coordinator has none.
- **`Dockerfile`** — multi-stage, distroless `cc-debian12:nonroot` runtime.

## systemd (bare VPS)

```sh
cargo build --release -p coordinator-bin
sudo install -m 0755 target/release/coordinator /usr/local/bin/ducktape-coordinator
sudo install -D -m 0644 ops/coordinator/coordinator.env.example /etc/ducktape/coordinator.env  # edit the bind addr
sudo cp ops/coordinator/ducktape-coordinator.service /etc/systemd/system/
sudo systemctl daemon-reload && sudo systemctl enable --now ducktape-coordinator
```

## Docker / OCI

```sh
docker build -f ops/coordinator/Dockerfile -t ducktape-coordinator .
docker run --rm -p 3478:3478/udp ducktape-coordinator
```

**Relay caveat:** a `--listen 0.0.0.0:3478` bind makes ciphertext-relay grants
undialable (`0.0.0.0:<port>`). Bind the routable public IP if you need relay
fallback. STUN, rendezvous, and hole-punch are unaffected. Full detail in
`docs/deploy/coordinator.md`.
