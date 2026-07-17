# Execution / Auth Separation — Trustless Credential Gateway v2

Promotes the `poc/trustless-gateway` PoC into the target architecture: separate
*who runs the agent* (Computation Provider) from *who holds the credential*
(Credential Provider ≡ Gateway/TEE). Same package, real crypto, so it can later
graft onto the production broker seams in
`crates/system/capability-host/src/broker.rs`.

Supersedes the role sketch in
`2026-07-18-trustless-credential-gateway-poc-design.md` (which this refines, not
replaces — the sealing/token/attestation core stands).

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
`tcg-host` on loopback) and `RemoteGateway` (a remote node reached via the local
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
   `shared = ECDH(eph_sk, seal_pk)`, `session_key = HKDF(shared, "tcg-session-v1")`.
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

Core (`tcg-core`, pure, no IO/async):
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

Host (`tcg-host serve`):
- `--attest mock|tdx|snp|auto`; generic `tsm_gen_quote` for tdx/snp.
- `/session` runs the enclave side of the handshake and returns the sealed token.

Client (`tcg-client`):
- `Gateway` abstraction: `--host <url>` (local) or `--remote <handle>.duck --via
  <browser-gw-url>` (remote). Adds the `x-duck-authority` header for remote.
- `seal` / `run` / `token` do: attest → verify (per vendor) → handshake → open
  token → use.

## Graft onto production `broker.rs` (integration slice, not this PR)

The seams are already there; this PoC is shaped to drop onto them:
- `AnthropicAuth::from_host()` is the **Local** credential source. Add a
  **Remote** arm that, instead of reading local creds, forwards each proxied
  request to a `RemoteGateway` (duckdns handle) carrying the session token.
- `Reachability {Loopback, HostGateway}` already picks the child's dial address;
  a third dimension (credential *locality*: local vs remote-TEE) is orthogonal
  and composes with it.
- `apply_auth_env` already hands the child `ANTHROPIC_BASE_URL` + an opaque
  bearer — unchanged. The broker gains a `CredentialGateway` it dispatches to.
- The overlay hop reuses the gateway `RouteTarget::LoopbackHttp` +
  `/v1/gateway/proxy` path (`bin/noded/src/gateway_http.rs`). Note: that proxy
  **buffers** responses today; SSE streaming needs the WS-upgrade lane or a
  streaming extension to `read_proxy_response` — the deferred transport slice.

## Out of scope (later specs)

Body-level AEAD of proxied traffic, SSE-over-overlay streaming, the actual
`broker.rs` graft, revocation, multi-tenant budgets, sealed-to-disk credential
persistence. Subscription-OAuth proxied by a third party remains an accepted,
named ToS risk — TEE custody is mitigation, not a solution.
