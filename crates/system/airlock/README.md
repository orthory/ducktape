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

## Credential the gateway holds

`airlock-cli seal` uploads one of two sealed credentials (`CredentialPayload`):

- **`Refresh`** — an OAuth refresh token; the gateway exchanges it for an access
  token and **rotates** on each refresh (subscription path).
- **`Bearer`** — a **static** access token, used as-is: no refresh, no rotation.
  `seal --credentials <file> --cred-kind bearer` seals a live subscription's
  *current* access token without invalidating the token chain its owner is still
  using — the safe way to point a run at a real credential.

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
swap → reply — asserting the session token never reaches the upstream (and that a
static `Bearer` credential is used without any OAuth refresh).

### Verified end-to-end against the real API

The whole chain has run live (mock attest, real `api.anthropic.com`):

1. `airlock-gateway serve --anthropic-base https://api.anthropic.com` (loopback).
2. `airlock-cli seal --credentials ~/.claude/.credentials.json --cred-kind bearer`
   — seals the current subscription access token (no rotation).
3. A `claude` CLI inside a **podman** sandbox holding only the opaque run bearer,
   driven through `capability-host` in airlock mode
   (`DUCKTAPE_AIRLOCK_GATEWAY`/`_MEASUREMENT`/`_ATTEST`) — the ignored live test
   `claude_model_turn_through_the_broker`.

`podman(claude) → capability-host broker → airlock gateway → real api.anthropic.com`
returned a real completion; the sandbox never held the credential, only the
temp bearer.


## Grafted into the product

`capability-host`'s Anthropic broker can now use a verified airlock gateway as
its credential SOURCE instead of a host-held credential: set
`DUCKTAPE_AIRLOCK_GATEWAY=<url>` (local) or `DUCKTAPE_AIRLOCK_REMOTE=<handle>.duck`
+ `DUCKTAPE_AIRLOCK_VIA=<browser-gw>` (remote), plus `DUCKTAPE_AIRLOCK_MEASUREMENT`
(the pinned audited-image hex) and `DUCKTAPE_AIRLOCK_ATTEST` (`mock` — dev only,
forgeable — or `tdx`/`snp`; no default, so nobody silently gets mock). The run's
`claude` traffic is then verified, handshaked, and forwarded to the gateway with
a scoped session token (re-minted on a gateway 401). The local path is exercised
end-to-end by in-process tests (`cargo test -p capability-host airlock`),
including a check that a sandbox child cannot inject the overlay routing header.
See the design spec §graft.

## Deferred

Body-level AEAD of proxied traffic, and **SSE-over-overlay streaming** — the
remote topology routes through the node's gateway proxy, which today BUFFERS
responses (4 MiB cap; only the WS-upgrade lane streams), so live `claude`
streaming over the overlay waits on that slice. Remote mode also needs the node
to publish the gateway's `LoopbackHttp` route with `allow_authorization` so the
session-token bearer reaches the enclave. Subscription OAuth proxied by a third
party remains an accepted, named ToS risk — attested custody is mitigation, not
a solution.
