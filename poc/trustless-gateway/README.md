# Trustless Credential Gateway — PoC

Prove that an agent sandbox can call the Claude API using a credential it never
holds, brokered by a TEE exit node whose own operator cannot read the
credential out of it.

Standalone PoC — not wired into the main ducktape workspace. Design:
`docs/superpowers/specs/2026-07-18-trustless-credential-gateway-poc-design.md`.

## Roles

- **Credential Provider** (`tcg-client seal`) — verifies the enclave quote,
  then seals + uploads the OAuth refresh token. Released only after the quote
  proves the audited image.
- **Trustless Gateway** (`tcg-host serve`) — runs in an Intel TDX confidential
  VM. Holds the sealed credential in enclave memory, proxies `/v1/messages`,
  issues scoped session tokens.
- **Computation Provider** (`tcg-client run`) — gets a scoped session token,
  makes proxied API calls with it. Never holds the credential.

## Two mock axes

| flag | mock | real |
|------|------|------|
| `--attest` | fake quote, runs anywhere | TDX quote via configfs-tsm, verified with `dcap-qvl` (`tcg-client --features tdx`) |
| upstream (`--anthropic-base`/`--oauth-token-url`) | `tcg-host mock-upstream` | `console.anthropic.com` + `api.anthropic.com` |

Best trust demo: `--attest tdx` + mock upstream (real enclave, zero ToS
exposure). Hermetic dev demo: both mock.

## Run (hermetic, any box)

```sh
./demo.sh              # mock attest + mock upstream, asserts a reply through the enclave
./demo-claude-code.sh  # the REAL `claude` CLI runs through the enclave with only a temp session token
```

`demo-claude-code.sh` drives the real Claude Code CLI against the host with
`ANTHROPIC_BASE_URL=<host>` + `ANTHROPIC_AUTH_TOKEN=<session token>`. The mock
upstream emits a valid Anthropic SSE, so it is fully local (no real Anthropic,
no ToS exposure) yet exercises the whole custody path: seal → temp token →
Claude Code → host swaps token→credential → reply. To point at real Anthropic
(spends subscription; account-sharing exposure): set `UPSTREAM_BASE`,
`OAUTH_URL`, `CREDS` (see the script header).

## Run (full scenario on an Intel TDX box)

Run **inside the TD guest**:

```sh
./demo-tdx.sh    # real configfs-tsm quote + dcap-qvl verify, mock upstream
```

Prereqs in the guest: kernel ≥ 6.7 with `configfs-tsm`
(`/sys/kernel/config/tsm/report` present); a working quote-generation path (QGS
over vsock — a bare TDX guest without it returns a *report*, not a verifiable
*quote*); network egress to Intel PCS (or set `PCCS_URL`); root (configfs report
dirs). Build the client with `--features tdx`.

The measurement bootstrap is a chicken-and-egg: `seal` needs the MRTD to pin.
`tcg-client inspect --attest tdx` reads it out of the quote (MRTD → stdout,
RTMRs + REPORTDATA → stderr). `demo-tdx.sh` does this as TOFU; **in production
pin the MRTD from the audited build**, not by reading it back from the quote you
are verifying.

To also validate the real OAuth constants against Anthropic (spends real
subscription; account-sharing exposure):

```sh
UPSTREAM_BASE=https://api.anthropic.com \
OAUTH_URL=https://console.anthropic.com/v1/oauth/token \
CREDS=$HOME/.claude/.credentials.json ./demo-tdx.sh
```

Note: `tdx_verify` is best-effort against `dcap-qvl` 0.3 (compiles clean with
`--features tdx`); the report-vs-quote / QGS path is the most likely thing to
need adjusting per platform.

## Out of scope (later specs)

duckdns/gateway/overlay transport, SSE-over-overlay streaming, revocation,
multi-tenant budgets, sealed-to-disk persistence. Subscription-OAuth proxied by
a third party is an accepted, named ToS risk — TEE custody is mitigation, not a
solution.
