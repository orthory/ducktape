# airlock

The barrier between **execution** and **auth**. An agent sandbox runs on one
side; the OAuth/API credential lives on the other. Nothing crosses without an
attested, session-scoped handshake — so the sandbox calls the Claude/Codex API
using a credential it never holds, and the operator of the credential side
cannot read the credential out of it.

Promoted from a PoC into real workspace crates. Design:
`docs/superpowers/specs/2026-07-18-execution-auth-separation-design.md`.

## Crates

- **`airlock`** (this crate) — the pure attestation + sealing + session-key
  handshake core (`attest`, `seal`, `handshake`, `token`, `wire`, `aead`), plus
  the async client (`client::Gateway`) behind the `client` feature. Off-consensus;
  uses RustCrypto primitives directly like `reachability`.
- **`airlock-gateway`** (`bin/`) — the credential side (the TEE). Runs
  canonically inside an Intel TDX / AMD SEV-SNP confidential VM. Holds a sealed
  refresh token in enclave memory, attests, proxies `/v1/*`, issues scoped
  session tokens.
- **`airlock-broker`** (`bin/`) — the Computation Provider's local api-snatch.
  Verifies the gateway quote, handshakes once, and exposes a loopback
  `ANTHROPIC_BASE_URL` for an unmodified sandbox (the real `claude` CLI). The
  sandbox holds only an opaque per-run bearer.
- **`airlock-cli`** (`bin/`) — client roles: `seal` (Credential Provider),
  `inspect` (pin the measurement), `run` (self-test).

## Two topologies

| topology | when | broker/cli flags |
|----------|------|------------------|
| **Local** | Credential Provider == Computation Provider | `--gateway-host <url>` / `--host <url>` |
| **Remote** | Credential Provider != Computation Provider | `--remote <handle>.duck --via <browser-gw>` |

Remote adds an `x-duck-authority` header; the local node's browser-gateway routes
it to the remote node's published `LoopbackHttp` route over the overlay.

## Session-key handshake

The client reads the gateway's `seal_pk` out of the **verified** quote REPORTDATA,
ECDHs against it, and derives a session key. `/session` returns the token
**AEAD-sealed under that key**, so only a client that talked to the *attested*
enclave can open it — a relaying node operator cannot substitute its key or read
the token.

## Per-vendor attestation (`--attest mock|tdx|snp|auto`)

Quote generation is vendor-generic via `configfs-tsm` (`tdx_guest`/`sev_guest`;
`auto` probes the provider). Verification is vendor-specific: `mock` (always),
`tdx` (`dcap-qvl`, `airlock-cli --features tdx`), `snp` (structural; AMD KDS/VCEK
verify is the follow-up, `--features snp`, fails closed).

## Test

```sh
cargo test -p airlock --features server,client
```

Unit tests cover the crypto (seal/handshake/token/attest); the in-process e2e
test (`tests/e2e.rs`) boots the gateway server against a mock upstream and drives
the full custody path — attest → seal → handshake → proxied call → credential
swap → reply — asserting the session token never reaches the upstream.


## Deferred

Body-level AEAD of proxied traffic, SSE-over-overlay streaming, wiring
`airlock-broker`'s Remote mode as the default credential source inside
`capability-host` (the seam is mapped in the design spec §graft). Subscription
OAuth proxied by a third party remains an accepted, named ToS risk — attested
custody is mitigation, not a solution.
