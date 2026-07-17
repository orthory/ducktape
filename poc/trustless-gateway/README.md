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

## Run

```sh
./demo.sh        # hermetic: mock attest + mock upstream, asserts a reply
```

On the TDX box, run `tcg-host serve --attest tdx …` inside the guest and
`tcg-client seal --attest tdx --measurement <real MRTD>` with
`tcg-client` built `--features tdx`.

## Out of scope (later specs)

duckdns/gateway/overlay transport, SSE-over-overlay streaming, revocation,
multi-tenant budgets, sealed-to-disk persistence. Subscription-OAuth proxied by
a third party is an accepted, named ToS risk — TEE custody is mitigation, not a
solution.
