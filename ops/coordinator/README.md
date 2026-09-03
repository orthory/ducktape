# `ops/coordinator/` — deploy artifacts for the untrusted coordinator

Three ready-to-use artifacts for running `bin/coordinator` as
`relay.ducktape.industries`. The coordinator **holds no keys, serves no state,
and is untrusted by design**; the operator recipe, the trust model, the auth
modes and the load/soak probes are all in `docs/deploy/coordinator.md`.

- **`ducktape-coordinator.service`** — hardened systemd unit (DynamicUser,
  `CAP_NET_BIND_SERVICE` only — the relay lane binds 443 — read-only
  filesystem, Internet IP sockets only).
- **`coordinator.env.example`** — the single operator-edited line, the bind
  address, plus optional auth-mode args. **Not a secret**; the coordinator has
  none.
- **`Dockerfile`** — multi-stage, distroless `cc-debian12:nonroot` runtime.
