# Trustless Credential Gateway — PoC design

Date: 2026-07-18
Status: approved for PoC build

## Goal

Prove, end to end, that an agent sandbox can call the Claude API using a
credential it never holds, brokered by a TEE exit node that its own operator
cannot read the credential out of. This is a standalone PoC in a separate
package — no integration into the existing broker / dispatch / gateway code
yet. That integration is a later spec.

## Three roles (PoC collapses two into one binary)

- **Credential Provider** — logs in, seals the OAuth refresh token to the
  enclave, uploads it. Never runs compute.
- **Trustless Gateway (host)** — runs inside an Intel TDX confidential VM.
  Holds the sealed credential in enclave memory, proxies Claude API, issues
  scoped session tokens. The host operator cannot read the credential.
- **Computation Provider** — handshakes with the gateway, gets a scoped
  session token, uses it as the bearer for proxied API calls. Never holds the
  credential.

The PoC has no separate machines to prove *identity* of the two client roles,
so both live in one `tcg-client` binary as subcommands (`seal`, `run`). The
`host` is its own binary and runs (canonically) in the TDX guest.

## Package layout

Standalone Cargo workspace, **outside** the main 48-crate build so it carries
none of the repo gates or build weight:

```
poc/trustless-gateway/
  Cargo.toml            # workspace: core, host, client
  core/                 # lib: wire types, sealing, Attestor/Verifier, session token, mock upstream
  host/    -> tcg-host  # gateway; also a `mock-upstream` subcommand for the hermetic demo
  client/  -> tcg-client# subcommands: seal (Credential Provider), run (Computation Provider)
  demo.sh               # hermetic end-to-end: --attest mock --upstream mock
```

## The flow

### Host boot (inside the TD)

1. Generate, in enclave memory only:
   - `seal` keypair (X25519) — recipients seal the credential to `seal_pk`.
   - `sess` keypair (Ed25519) — signs session tokens.
2. `REPORTDATA = seal_pk(32) ‖ sess_pk(32)` — exactly 64 bytes, so the TDX
   quote *is* the key binding. No hashing, no trusting a JSON field: the
   verifier extracts both pubkeys out of the verified quote.
3. Request a TDX quote for that REPORTDATA (`configfs-tsm`).
4. Serve HTTP:
   - `GET /attestation` → `{ quote_b64 }`.
   - `POST /credential` → sealed blob → unseal **inside the TD** → hold
     `{access_token, refresh_token, expires_at}` in a `Mutex`; do an initial
     OAuth refresh so an access token is ready.
   - `POST /session` → issue an Ed25519-signed token `{sub, iat, exp,
     max_requests}`.
   - `POST /v1/messages` → require `Authorization: Bearer <session_token>` →
     verify signature/exp/budget → `refresh_if_needed` → proxy to the Anthropic
     messages endpoint with the real access token as `Authorization: Bearer`,
     SSE streamed straight through.

### `tcg-client seal` (Credential Provider)

1. `GET /attestation`.
2. **Verify the TDX quote**: Intel signature chain + `MRTD`/`RTMR` equals the
   pinned expected measurement (the audited host image).
3. Extract `seal_pk` from the verified REPORTDATA.
4. Read the local refresh token (`~/.claude/.credentials.json`
   `claudeAiOauth.refreshToken`, or a literal passed on the CLI for the mock
   run).
5. Seal it to `seal_pk`, `POST /credential`.

The token is released **only after** the quote proves it is the audited image.

### `tcg-client run` (Computation Provider)

1. `POST /session` → session token.
2. Prove two ways:
   - direct `POST /v1/messages` with a small prompt, assert a reply;
   - (manual/optional) drive the real `claude` CLI with
     `ANTHROPIC_BASE_URL=<host>` + `ANTHROPIC_AUTH_TOKEN=<session_token>` to
     show the unchanged broker-consumer path works against a remote enclave.

## Two independent mock axes

- `--attest mock|tdx`
  - `mock` — fake quote (`b"MOCK" ‖ report_data ‖ ed25519_sig`), matching
    verifier. Runs on any box (dev is a Ryzen 5950X with no SEV/TDX).
  - `tdx` — `configfs-tsm` quote generation + `dcap-qvl` verification. Runs on
    the coworker's TDX machine. Behind a `tdx` cargo feature so the default
    build stays light.
- `--upstream mock|real`
  - `mock` — `core::mock_upstream`: `POST /oauth/token` returns
    `{access_token: "acc-<n>", refresh_token: "ref-<n+1>", expires_in}` and
    **rotates the refresh token every call** (exercises the refresh path and
    the memory-only "lost on restart" behavior); `POST /v1/messages` checks
    `Authorization: Bearer acc-<n>` (the current access token, NOT the session
    token) and streams a fake SSE reply. The bearer check is the load-bearing
    assertion: if the host failed to swap `session_token → access_token`, the
    mock 401s.
  - `real` — `console.anthropic.com` OAuth + `api.anthropic.com/v1/messages`.
    One-time live pass to validate the OAuth constants (client id + token URL
    are marked "PENDING live validation" in the current broker).

The best trust demo is `--attest tdx --upstream mock`: real enclave, zero ToS
exposure. The hermetic dev demo is `--attest mock --upstream mock`.

## Crate choices (pure Rust, no new C dependency)

- Sealing: NaCl sealed box (`dryoc`, i.e. `crypto_box_seal`) — recipient
  pubkey only, anonymous sender. HPKE base mode (`hpke`) is the fallback if the
  sealed-box API is awkward.
- Session token: `base64url(json) . base64url(ed25519_sig)` via
  `ed25519-dalek`. No JWT library.
- TDX quote generation: raw `configfs-tsm` filesystem IO (kernel ≥ 6.7) — no
  crate needed on the guest.
- TDX quote verification: `dcap-qvl` (pure-Rust DCAP against Intel PCS
  collateral) — avoids the Intel SGX-DCAP C stack.
- HTTP + proxy: `axum` + `reqwest` (same shape as the current broker), SSE via
  `Body::from_stream(resp.bytes_stream())`.

## Attestation seam

```rust
// host side
trait Attestor { fn quote(&self, report_data: [u8; 64]) -> anyhow::Result<Vec<u8>>; }
// client side — returns the verified REPORTDATA (from which pubkeys are read)
trait Verifier { fn verify(&self, quote: &[u8], expected: &Measurement) -> anyhow::Result<[u8; 64]>; }
```

`MockAttestor`/`MockVerifier` are always built. `TdxAttestor` (configfs-tsm)
and `TdxVerifier` (dcap-qvl) live behind `--features tdx`. The expected
measurement is a PoC CLI flag; production pins it on Ducktape consensus.

## What the PoC proves

- The host operator cannot read the refresh token: sealed to an in-TD key,
  memory only, decrypted only inside the TD.
- The Credential Provider releases the token only against a verified
  audited-image quote.
- The Computation Provider uses the credential without ever holding it: the
  session token is scoped and expiring and is not the credential; the host
  swaps it for the real access token upstream.

## Explicitly out of scope (later specs)

- duckdns + gateway `RouteRecord` + overlay transport (PoC is plain HTTP with
  transport behind a small interface).
- SSE streaming over the gateway overlay proxy (`MAX_PROXY_FRAME_BYTES` gap).
- Revocation, multi-tenant budgets, per-account policy.
- Sealed-to-disk persistence across TD restart (memory-only → re-seal on
  restart).
- Side-channel resistance.
- The account-sharing ToS reality: subscription OAuth proxied by a third-party
  TEE is the pattern Anthropic enforces server-side since Jan 2026. TEE custody
  is the mitigation story, not a solution. **Named, accepted risk.**

## Testing

- `core` unit tests: seal round-trip, session-token sign/verify (tamper →
  reject), mock attest quote→verify round-trip, mock-upstream refresh rotation.
- `demo.sh`: hermetic end-to-end (`--attest mock --upstream mock`) — boot
  mock-upstream, boot host at it, `client seal`, `client run`, assert the
  `/v1/messages` reply. Fails loudly if seal / verify / token / swap / proxy
  breaks.
- Real-TDX pass: the same `demo.sh` with `--attest tdx` on the coworker's box.
```
