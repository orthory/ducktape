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

## Remote overlay (cred ≠ compute) — how to run

The remote topology needs **no airlock or node code** beyond what ships here: the
compute side already speaks it, and the credential node exposes the gateway with
the stock gateway-route CLIs. `bin/node/tests/airlock_gateway_e2e.rs` is the
canonical, executable recipe (two real WireGuard nodes, in-process gateway + mock
upstream); run it where inline 2-node WireGuard works:

```sh
cargo test -p node-bin --test airlock_gateway_e2e -- --nocapture
```

For a real cross-machine deployment:

**Credential node** (runs the enclave gateway; `<handle>` is its duckdns name):

```sh
MEAS=<48-byte audited-image hex>            # mock: any 48-byte hex, pinned everywhere
airlock-gateway serve --attest mock --measurement $MEAS \
    --listen 127.0.0.1:9100 --anthropic-base https://api.anthropic.com
# seal the credential (static bearer = no rotation; see "Credential" above)
airlock-cli seal --attest mock --measurement $MEAS --host http://127.0.0.1:9100 \
    --credentials ~/.claude/.credentials.json --cred-kind bearer
# register the loopback port node-locally
ducktape gateway-route-bind --workspace <node-workspace> --label airlock --port 9100
# publish the signed LoopbackHttp route (allow_authorization:true, max_response_bytes ≤ 4 MiB
# — the buffered proxy enforces this cap literally; 0/"unbounded" awaits SSE-over-overlay);
# construct the RouteStatement exactly as signed_airlock_route() does in the test,
# then: user-sign-gateway-route --key <user.key> --statement <json>  → submit to the
# node RPC (cmd:submit, target:"gateway"). Also SetHandle <handle> on duckdns.
```

**Compute node** (runs the sandbox): point the broker at the remote gateway — no
credential is held locally:

```sh
export DUCKTAPE_AIRLOCK_REMOTE=airlock.<handle>.duck
export DUCKTAPE_AIRLOCK_VIA=$(curl -s http://<this-node-rpc>/v1/gateway/browser | jq -r .base)
export DUCKTAPE_AIRLOCK_MEASUREMENT=$MEAS
export DUCKTAPE_AIRLOCK_ATTEST=mock
# then run a claude agent through capability-host (podman) as usual — the run's
# /v1/messages flow crosses the overlay to the enclave.
```

The quote is fetched + verified **over the overlay** before any token is derived,
so a relaying node cannot substitute its key or read the session token. Note the
overlay proxy currently **buffers** responses (4 MiB); live SSE streaming for long
interactive turns is the remaining transport slice (spec §graft "Remaining").

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
