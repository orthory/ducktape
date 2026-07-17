# Trustless Credential Gateway — PoC

Separate **who runs the agent** from **who holds the credential**. An agent
sandbox calls the Claude/Codex API using a credential it never holds, brokered
by a TEE exit node whose own operator cannot read the credential out of it.

Standalone PoC — not wired into the main ducktape workspace. Design:
`docs/superpowers/specs/2026-07-18-execution-auth-separation-design.md`
(refines `…-trustless-credential-gateway-poc-design.md`).

## Roles

- **Credential Provider ≡ Gateway (TEE)** (`tcg-host serve`) — logs in via
  OAuth, holds the sealed refresh token in enclave memory, attests its
  measurement, proxies `/v1/*`, issues scoped session tokens. Runs canonically
  inside an Intel TDX / AMD SEV-SNP confidential VM; the operator cannot read
  the credential.
- **Computation Provider** (`tcg-client run` / `token`) — runs the agent
  sandbox. Verifies the enclave quote, runs a session-key handshake, gets a
  scoped session token, makes proxied calls with it. Never holds the credential.

## Two topologies, one client abstraction

The sandbox always sees the same loopback broker (`ANTHROPIC_BASE_URL` + a
bearer). What changes is where the gateway lives:

| topology | when | client flags |
|----------|------|-------------|
| **Local** | Credential Provider == Computation Provider | `--host <url>` |
| **Remote** | Credential Provider != Computation Provider | `--remote <handle>.duck --via <browser-gw-url>` |

Remote adds an `x-duck-authority` header; the local node's browser-gateway
routes it to the remote node's published `LoopbackHttp` route over the overlay.

## Session-key handshake (both topologies)

The client reads the enclave's `seal_pk` out of the **verified** quote REPORTDATA,
ECDHs against it, and derives a session key. `/session` returns the token
**AEAD-sealed under that key**, so only a client that talked to the *attested*
enclave can open it — a relaying node operator cannot substitute its key or read
the token. (Body-level AEAD / SSE-over-overlay streaming is the deferred
transport slice.)

## Per-vendor TEE (`--attest`)

| value | quote gen (host) | verify (client) |
|-------|------------------|-----------------|
| `mock` | fake, well-known issuer, runs anywhere | always |
| `tdx` | `configfs-tsm` (`tdx_guest`) | `dcap-qvl` vs Intel PCS (`--features tdx`) |
| `snp` | `configfs-tsm` (`sev_guest`) | structural parse; AMD KDS/VCEK verify is the follow-up (`--features snp`, fails closed) |
| `auto` (host only) | probes `configfs-tsm` `provider` | — |

Quote generation is vendor-generic (the sysfs path is identical); only
verification is vendor-specific, so the default client build stays
dependency-light.

## Run (hermetic, any box)

```sh
./demo.sh              # LOCAL topology: mock attest + mock upstream, reply through the enclave
./demo-remote.sh       # REMOTE topology: same, reached via --remote/--via (x-duck-authority path)
./demo-claude-code.sh  # the REAL `claude` CLI runs through the enclave with only a temp session token
```

All three are fully local (the mock upstream emits a valid Anthropic SSE, so no
real Anthropic and no ToS exposure) yet exercise the whole custody path: attest
→ handshake → scoped token → host swaps token→credential → reply. To point at
real Anthropic (spends subscription; account-sharing exposure) set
`UPSTREAM_BASE`, `OAUTH_URL`, `CREDS` (see each script header).

## Run (full scenario on an Intel TDX box)

Run **inside the TD guest**:

```sh
./demo-tdx.sh    # real configfs-tsm quote + dcap-qvl verify, mock upstream
```

Prereqs in the guest: kernel ≥ 6.7 with `configfs-tsm`; a working
quote-generation path (QGS over vsock — a bare TDX guest without it returns a
*report*, not a verifiable *quote*); egress to Intel PCS (or set `PCCS_URL`);
root. Build the client `--features tdx`.

The measurement bootstrap is a chicken-and-egg: `seal`/`run` need the MRTD to
pin. `tcg-client inspect --attest tdx` reads it out of the quote.
`demo-tdx.sh` does this as TOFU; **in production pin the MRTD from the audited
build**, not by reading it back from the quote you are verifying.

## Graft onto production `broker.rs`

The seams (`crates/system/capability-host/src/broker.rs`) are mapped in the
design spec §graft: `AnthropicAuth::from_host` is the Local credential source;
a Remote arm forwards over the gateway `LoopbackHttp` route; `Reachability` and
`apply_auth_env` are unchanged. The overlay proxy **buffers** today, so SSE
streaming needs the WS-upgrade lane or a streaming extension — the deferred
transport slice.

## Out of scope (later specs)

Body-level AEAD of proxied traffic, SSE-over-overlay streaming, the actual
`broker.rs` graft, revocation, multi-tenant budgets, sealed-to-disk persistence.
Subscription-OAuth proxied by a third party is an accepted, named ToS risk —
TEE custody is mitigation, not a solution.
