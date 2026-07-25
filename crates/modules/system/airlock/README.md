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

**Credential node** (runs the enclave gateway; `<handle>` is its duckdns name).
The node **embeds** the gateway: set `DUCKTAPE_AIRLOCK_SERVE` and it runs the
gateway in-process, registers its loopback port as the `airlock` route, and seeds
the credential — no separate serve / bind / seal steps. The credential provider IS
the node process, so the credential is seeded directly (not sealed-uploaded).

```sh
# in the node's environment (systemd unit / launchd / shell). The node must run
# INSIDE a TDX/SNP confidential VM — there is no mock, a box that cannot attest
# cannot serve credentials:
export DUCKTAPE_AIRLOCK_SERVE=1
export DUCKTAPE_AIRLOCK_SERVE_ATTEST=auto           # or tdx|snp; REQUIRED, no default
export DUCKTAPE_AIRLOCK_SERVE_ANTHROPIC_BASE=https://api.anthropic.com
export DUCKTAPE_AIRLOCK_SERVE_CREDENTIALS=~/.claude/.credentials.json
export DUCKTAPE_AIRLOCK_SERVE_CRED_KIND=bearer     # static access token, no rotation
ducktape-node --config node.toml                  # gateway comes up + route registers at boot
# clients pin the audited image's measurement on THEIR side (--measurement /
# DUCKTAPE_AIRLOCK_MEASUREMENT + --snp-product); the serving node takes none.
# then the ONE manual, signed ownership act — publish the LoopbackHttp route
# (allow_authorization:true; max_response_bytes 0 = unbounded live stream, the
# right choice for claude SSE). Build the RouteStatement exactly as
# signed_airlock_route() in the test, then:
#   ducktape-node user-sign-gateway-route --key <user.key> --statement <json>
#   → submit to the node RPC (cmd:submit, target:"gateway"). Also SetHandle <handle>.
```

The manual route (for a gateway NOT run by the node) still works and is what
the standalone binaries are for:

```sh
MEAS=<48-byte audited-image hex>            # pinned from the audited CVM image
airlock-gateway serve --attest snp --listen 127.0.0.1:9100 \
    --anthropic-base https://api.anthropic.com     # must run IN a TDX/SNP guest
# seal the credential (static bearer = no rotation; see "Credential" above)
airlock-cli seal --attest snp --snp-product milan --measurement $MEAS \
    --host http://127.0.0.1:9100 \
    --credentials ~/.claude/.credentials.json --cred-kind bearer
# register the loopback port node-locally
ducktape gateway bind --workspace <node-workspace> --label airlock --port 9100
# publish the signed LoopbackHttp route (allow_authorization:true, max_response_bytes ≤ 4 MiB
# — the buffered proxy enforces this cap literally; 0/"unbounded" awaits SSE-over-overlay);
# construct the RouteStatement exactly as signed_airlock_route() does in the test,
# then: user sign-gateway-route --key <user.key> --statement <json>  → submit to the
# node RPC (cmd:submit, target:"gateway"). Also SetHandle <handle> on duckdns.
```

**Compute node** (runs the sandbox): point the broker at the remote gateway — no
credential is held locally:

```sh
export DUCKTAPE_AIRLOCK_REMOTE=airlock.<handle>.duck
export DUCKTAPE_AIRLOCK_VIA=$(curl -s http://<this-node-rpc>/v1/gateway/browser | jq -r .base)
export DUCKTAPE_AIRLOCK_MEASUREMENT=$MEAS
export DUCKTAPE_AIRLOCK_ATTEST=snp                  # or tdx
export DUCKTAPE_AIRLOCK_SNP_PRODUCT=milan           # snp: pin the platform generation
# optional transport overrides: DUCKTAPE_AIRLOCK_SNP_VCEK=<der file> (air-gapped),
# DUCKTAPE_AIRLOCK_PCCS_URL=<pccs> (tdx)
# then run a claude agent through capability-host (podman) as usual — the run's
# /v1/messages flow crosses the overlay to the enclave.
```

The quote is fetched + verified **over the overlay** before any token is derived,
so a relaying node cannot substitute its key or read the session token. The
overlay proxy **streams** responses end to end (2026-07-20): publish the route
with `max_response_bytes: 0` (unbounded stream — now literal) for live SSE; a
non-zero cap is enforced as a RUNNING total (declared over-length refused
before the head; unsized overflow truncates the body mid-stream). The
request-body admission ceiling is 16 MiB.

## Body AEAD (sealed sessions)

Airlock sessions are **sealed-body** (2026-07-20): the broker seals request
bodies and unseals response streams under keys derived from the same handshake
ECDH (`bodyseal`; per-stream salted key, counter nonces, authenticated Final
marker). Consequences:

- **Path hosts see ciphertext** — including the credential-provider's node
  process OUTSIDE the enclave (`proxy_loopback` relays opaque bytes), closing
  the "operator reads conversation content" gap.
- **A stolen bearer alone is useless**: the enclave refuses plaintext bodies
  on a sealed session, and a plaintext success reply is refused by the broker
  as forgery. Missing Final marker = authenticated truncation (aborts, never a
  clean EOF).
- The sandbox is unmodified — the broker terminates the AEAD.
- Session revocation = gateway restart (the token-signing and seal keys are
  memory-only, so every outstanding token and body key dies with the process);
  a per-sub revocation endpoint waits for real multi-tenancy.

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

## Per-vendor attestation (`--attest tdx|snp`, gateway also `auto`)

Quote generation is vendor-generic via `configfs-tsm` (`tdx_guest`/`sev_guest`;
`auto` probes the provider). Verification (`airlock::verify`, feature `verify`)
is real and fails closed — there is NO mock:

- **`tdx`** — full DCAP verification via `dcap-qvl` (PCK chain anchored to the
  Intel root pinned inside the crate, TCB info, QE identity, quote signature).
  Collateral comes from Intel PCS (`--pccs-url`/`DUCKTAPE_AIRLOCK_PCCS_URL`
  overrides the endpoint, never the root). TCB status must be `UpToDate` or
  `SWHardeningNeeded`; anything else is refused by name.
- **`snp`** — full AMD chain via the `sev` crate (`crypto_nossl`): the VCEK
  (fetched from AMD KDS by chip id + reported TCB, cached; or supplied as a DER
  file) must chain to the **builtin AMD ARK/ASK for the operator-pinned product
  generation** (`--snp-product milan|genoa|turin`), and the report signature
  must verify under it. VLEK-signed reports are refused. The measurement is then
  compared to the pinned value. Known gap (deliberate, tracked with the
  hardware TODO): no TCB-freshness gate — AMD issues VCEKs for older TCBs, so
  firmware rollback is not detected; TDX gets freshness from its TCB status.

`--attest` has **no default** anywhere. Trust roots are plain typed data in the
lib (`TrustRoots`); each binary parses its flags/env ONCE at its boundary — the
airlock crate itself never reads the environment.

**Dev boxes without TEE silicon** use `airlock::testkit` (feature `testkit`):
`SnpTestEnclave` mints a real-format SNP report signed by a freshly generated
test chain, the server takes it via the `build_with_quoter` seam, and clients
verify it through the REAL verifier under `enclave.roots()`. A minted quote
never verifies under the AMD builtins, so nothing here weakens production —
injecting fake roots only fools the injector. Real vendored fixtures
(`tests/fixtures/`: an Intel-signed TDX quote + collateral, an AMD-signed Milan
report + VCEK) prove the production chains offline.

## Test

```sh
cargo test -p airlock --features server,client,verify,testkit
```

Unit tests cover the crypto (seal/handshake/token/attest); the in-process e2e
test (`tests/e2e.rs`) boots the gateway server against a mock upstream and drives
the full custody path — attest → seal → handshake → proxied call → credential
swap → reply — asserting the session token never reaches the upstream (and that a
static `Bearer` credential is used without any OAuth refresh).

### Verified end-to-end against the real API

The whole chain has run live (2026-07-19, real `api.anthropic.com`, with the
since-deleted mock attest standing in for the quote — the custody path is
unchanged; a TEE-silicon rerun is the standing hardware TODO):

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
(the pinned audited-image hex) and `DUCKTAPE_AIRLOCK_ATTEST` (`tdx`/`snp`; no
default). For snp, `DUCKTAPE_AIRLOCK_SNP_PRODUCT` pins the platform generation
(optional `DUCKTAPE_AIRLOCK_SNP_VCEK` file, `DUCKTAPE_AIRLOCK_PCCS_URL` for
tdx) — all parsed once at the config boundary. The run's
`claude` traffic is then verified, handshaked, and forwarded to the gateway with
a scoped session token (re-minted on a gateway 401). The local path is exercised
end-to-end by in-process tests (`cargo test -p broker-host airlock`),
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
