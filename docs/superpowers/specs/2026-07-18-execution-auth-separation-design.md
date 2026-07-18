# Execution / Auth Separation — airlock

Separate *who runs the agent* (Computation Provider) from *who holds the
credential* (Credential Provider ≡ Gateway/TEE). Nothing crosses the airlock
without an attested, session-scoped handshake.

## Implementation (promoted feature)

Shipped as real workspace crates (was a `poc/trustless-gateway` prototype):

- **`crates/system/airlock`** — pure attest + seal + handshake + token + wire +
  aead core; `client::Gateway` behind the `client` feature; the gateway HTTP
  service `server` behind the `server` feature. Off-consensus, RustCrypto
  primitives directly (like `reachability`). Unit tests + an in-process e2e test
  (`cargo test -p airlock --features server,client`).
- **`bin/airlock-gateway`** — the TEE credential side (thin wrapper over
  `airlock::server`).
- **`bin/airlock-broker`** — the Computation Provider's local api-snatch: a
  loopback `ANTHROPIC_BASE_URL` for an unmodified sandbox, forwarding to the
  gateway with a handshake-minted session token.
- **`bin/airlock-cli`** — `seal` (Credential Provider), `inspect`, `run`.

The role names below map to these crates. Sealing/token/attestation design is
unchanged from `2026-07-18-trustless-credential-gateway-poc-design.md`.

## Roles

- **Credential Provider ≡ Gateway (TEE).** One entity for now. Logs in via
  OAuth, holds the refresh token in enclave memory, attests its measurement,
  proxies the Claude/Codex API. The host operator cannot read the credential.
- **Computation Provider.** Runs the podman sandbox (`claude` / `codex`).
  Reaches the gateway through one abstraction with two topologies. Never holds
  the credential — only a scoped, expiring session token.

## Two topologies, one abstraction (the "api snatch" abstraction)

The podman sandbox always sees the same loopback broker
(`ANTHROPIC_BASE_URL` + an opaque bearer) — that interface does not change. What
changes is where the broker routes behind it:

| topology | when | transport |
|----------|------|-----------|
| **Local gateway** | Credential Provider == Computation Provider | loopback (the existing `broker.rs` way) |
| **Remote gateway** | Credential Provider != Computation Provider | duckdns handle → gateway `LoopbackHttp` route over the overlay |

In the PoC this is the client-side `Gateway` trait: `LocalGateway` (a same-host
`airlock-gateway` on loopback) and `RemoteGateway` (a remote node reached via the local
node's browser-gateway with `x-duck-authority: <handle>.duck`). Remote runs over
plain HTTP now; the overlay hop is the integration slice below.

## Session-key handshake (both topologies)

```
credential provider ── oauth ─────────────► TEE            (seal refresh token)
computation provider ── handshake ─────────► TEE ── session key (tls) ──►
```

The enclave's `seal_pk` (X25519, already bound in the attested REPORTDATA) doubles
as its static ECDH key — no new key, no extra REPORTDATA field.

1. Client `GET /attestation` → quote. Verify it (per vendor). Read
   `seal_pk ‖ sess_pk` out of the **verified** REPORTDATA — never a JSON field.
2. Client generates an ephemeral X25519 keypair, computes
   `shared = ECDH(eph_sk, seal_pk)`, `session_key = HKDF(shared, "airlock-session-v1")`.
3. `POST /session { sub, client_eph_pk }`. Enclave computes the same
   `session_key = HKDF(ECDH(seal_sk, client_eph_pk), …)`, issues the scoped token,
   and returns it **AEAD-sealed under `session_key`**.
4. Only a client that verified the quote and derived the key from the *attested*
   `seal_pk` can open the token. This binds the session to the attested enclave —
   a malicious remote node operator cannot substitute its own key or read the
   token in flight.

`session_key` is the channel key ("tls" in the flow diagram). This slice uses it
to protect the token handoff; body-level AEAD / SSE-over-overlay streaming is the
transport slice (deferred, see Out of scope). For loopback the key is
belt-and-suspenders; for remote it is the load-bearing attestation binding.

## Per-vendor TEE, feature-flagged

`attest::AttestMode { Mock, Tdx, Snp }`, selected by `--attest`
(`mock|tdx|snp|auto`); `auto` probes the platform.

- **Quote generation (host) is vendor-generic** via Linux `configfs-tsm`
  (`/sys/kernel/config/tsm/report/*`): write `inblob` = REPORTDATA(64), read
  `outblob` = raw report/quote. The kernel `provider` attribute is `tdx_guest`
  on Intel TDX and `sev_guest` on AMD SEV-SNP. `auto` reads `provider` to pick
  the vendor. No feature flag needed to *generate* — the sysfs path is the same.
- **Verification (client) is vendor-specific**, behind feature flags so the
  default build stays dependency-light:
  - `mock` — always, well-known issuer key, runs anywhere.
  - `tdx` (`--features tdx`, `dcap-qvl`) — verify against Intel PCS collateral,
    read MRTD from the TD10 report.
  - `snp` (`--features snp`) — verify the SEV-SNP attestation report against the
    AMD VCEK/KDS chain, read the launch measurement. PoC ships the structural
    parse + measurement compare with the cert-chain verify stubbed and clearly
    marked (mirrors how the TDX arm started), because this box has neither TDX
    nor SEV-SNP silicon (Ryzen 5950X: only `sme`).
- Measurement sizes coincide: TDX MRTD and SNP launch measurement are both
  SHA-384 (48 bytes), REPORTDATA is 64 bytes — the wire is vendor-agnostic on
  sizes. `AttestationResponse` carries `vendor` so the client selects the
  matching verifier.

## Components

Core (`airlock`, pure, no IO/async):
- `attest` — `AttestMode` (+ Snp), `Measurement`, REPORTDATA pack/split, mock
  quote gen+verify. Real TDX/SNP verify stays in the client behind features
  (needs async + network + heavy deps).
- `handshake` (new) — `client_handshake(seal_pk) -> (eph_pk, session_key)`,
  `enclave_session_key(seal_kp, client_eph_pk) -> session_key`,
  `seal_token/open_token` (symmetric AEAD under the session key).
- `seal` — add `SealKeypair::ecdh(peer_pk)` so the enclave side can derive the
  session key from its static X25519 secret.
- `token`, `wire` — `SessionRequest{ sub, client_eph_pk_b64 }`,
  `SessionResponse{ sealed_token_b64 }`, `AttestationResponse{ quote_b64, vendor }`.

Host (`airlock-gateway serve`):
- `--attest mock|tdx|snp|auto`; generic `tsm_gen_quote` for tdx/snp.
- `/session` runs the enclave side of the handshake and returns the sealed token.

Client (`airlock-cli`):
- `Gateway` abstraction: `--host <url>` (local) or `--remote <handle>.duck --via
  <browser-gw-url>` (remote). Adds the `x-duck-authority` header for remote.
- `seal` / `run` / `token` do: attest → verify (per vendor) → handshake → open
  token → use.

## Graft onto production `broker.rs`

**Landed (broker side).** `capability-host`'s Anthropic broker gained an
`AnthropicAuth::Airlock` arm alongside `ApiKey`/`Oauth`. When
`DUCKTAPE_AIRLOCK_GATEWAY` (local) or `DUCKTAPE_AIRLOCK_REMOTE` + `_VIA` (remote)
is set — with `DUCKTAPE_AIRLOCK_MEASUREMENT` pinning the audited image — the
broker verifies the gateway quote, handshakes for a scoped session token, and
forwards `/v1/messages` to the gateway (which swaps the token for the real
credential in-enclave) instead of holding a local credential. It re-handshakes
once on a gateway 401. `authorize()` stamps the session token (and
`x-duck-authority` on the remote path); the existing `Reachability
{Loopback, HostGateway}` (child→broker dial) is orthogonal and unchanged, as is
`apply_auth_env` (the child still gets `ANTHROPIC_BASE_URL` + the opaque run
bearer). Covered by an in-process test: sandbox → broker → gateway → mock
upstream, asserting the credential swap. Vendor verify (tdx/snp) in the host
broker is refused for now (mock only), matching `airlock-broker`.

**Wired (route-publish + reachability).** The remote topology needs no airlock or
node code beyond what ships. The compute side reaches the gateway through the
node's browser-gateway origin (`via` = `http://127.0.0.1:<gateway_listen>`), which
resolves `x-duck-authority` and proxies over the overlay to the publisher's
`LoopbackHttp` route; a host process sends no `Origin` header so it passes the
only guard. The credential node exposes the gateway with the **stock gateway-route
CLIs** — `gateway-route-bind` (registers the loopback port node-locally) +
`user-sign-gateway-route` (signs a `RouteStatement` with `LoopbackHttp`,
`allow_authorization: true`, and a real `max_response_bytes` cap ≤ 4 MiB — the
buffered proxy enforces the cap literally, so `0`/"unbounded" 502s until the SSE
slice) submitted to the `gateway` module, plus duckdns `SetHandle`.
`bin/node/tests/airlock_gateway_e2e.rs` is the executable recipe: a single-node
self-serve test that runtime-proves the whole airlock-over-gateway path (route
publish → browser door → `allow_authorization` bearer forward → `proxy_loopback`
→ attest+handshake+swap → reply), **verified green**, plus a two-node WireGuard
test for the node-to-node overlay hop (runs where inline 2-node WireGuard peers
reliably). The README's "Remote overlay" section is the operator runbook.

**Remaining (SSE streaming only).** The overlay proxy **buffers** responses (4 MiB
cap; only the WS-upgrade lane streams), so live `claude` SSE for long interactive
turns needs the WS-upgrade lane or a streaming `read_proxy_response`. A short turn
fits the buffered path. Note: `simnode` cannot exercise the overlay — it carries
the gateway/duckdns *consensus modules* but no WireGuard/`data_plane` transport
(`handle.gateway == None`); the node-to-node overlay proxy requires two real
`bin/node` instances, not the deterministic `/v1` twin.

## Out of scope (later specs)

Body-level AEAD of proxied traffic, SSE-over-overlay streaming (see §graft
"Remaining"), revocation, multi-tenant budgets, sealed-to-disk credential
persistence. Subscription-OAuth proxied by a third party remains an accepted,
named ToS risk — TEE custody is mitigation, not
a solution.
