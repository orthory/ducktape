# SSE-over-overlay streaming + airlock body AEAD

**Date:** 2026-07-20
**Scope (user-approved):** (A) stream proxied LoopbackHttp responses through the
gateway overlay end to end; (B) raise the request-body admission ceiling to
16 MiB; (C) broker↔enclave body encryption so no node host on the path can read
conversation content. Deferred with reasons: session revocation (restart
already revokes absolutely — the token-signing key is memory-only), multi-tenant
budgets (a per-sub budget exists; there is one tenant).

## Why now

The airlock remote topology (cred ≠ compute) proxies `claude` traffic through
the node gateway, which today BUFFERS every response (4 MiB ceiling even for
routes that declared `max_response_bytes = 0` = streaming) and admits request
bodies only up to 1 MiB — long interactive turns fit neither direction. And the
publisher node's host process (`proxy_loopback`) sees request/response
plaintext plus the session bearer, which contradicts the trust story that the
credential-side operator can read nothing.

## A. Streaming through the gateway plane

The node-to-node wire needs NO change: responses already travel as
`ResponseHead(1)` / `BodyChunk(2)` / `End(3)` / `Failure(7)` frames
(`crates/networking/gateway/src/frames.rs`), and the WS-upgrade lane already
streams bidirectionally over the same overlay TCP stream. The work is removing
the three re-buffering points and unifying paths:

1. **Publisher** — `serve_current` returns `(head, BodyStream)` instead of a
   buffered `GatewayResponse`; `proxy_loopback` forwards
   `reqwest::Response::bytes_stream()` through a running-cap adaptor
   (`max_response_bytes > 0`: abort when the running total exceeds; `0`:
   unbounded — the declared-streaming case becomes real). The DuckFs arm wraps
   its existing buffered result in a one-chunk stream so there is ONE response
   shape. The server half writes `ResponseHead`, then per-chunk
   `BodyChunk` + flush, then `End`; a mid-stream upstream error or cap overflow
   emits `Failure` and closes (the client aborts the HTTP body — standard
   proxy truncation; whole-response error replies remain only for pre-head
   failures). Timeouts follow the WS lane: a deadline covers route resolution +
   loopback connect + response head; the body has none.
2. **Client** — `read_proxy_response` splits into head-read + a spawned frame
   pump feeding a bounded `mpsc` of `Bytes` (backpressure = the channel + the
   paced stream, exactly like the WS pump). The 4 MiB clamp on `0`-cap routes
   is deleted; per-frame size stays codec-bounded (`MAX_CHUNK_BYTES` 256 KiB).
3. **Doors** — `gateway_browser_proxy` serves `Body::from_stream`. The JSON
   API lane (`gateway_proxy`, `body_b64`) is buffered BY CONTRACT; it collects
   the stream with the old cap semantics and stays as-is.
4. **Self-serve unification** — publisher == self stops calling `serve_current`
   in-process and instead runs the same frame protocol over `tokio::io::duplex`
   (the mechanism the WS lane already uses for self-serve). One code path, and
   the single-node e2e on a non-WG box then exercises the REAL frame wire —
   today only the unreliable 2-node lane does.

`GatewayJob::Http`'s reply becomes head + body-stream; `GatewayResponse`
reshapes accordingly. In-place update per the no-backcompat mandate.

## B. Request-body ceiling 1 MiB → 16 MiB

`MAX_REQUEST_BODY_BYTES` (gateway `interface.rs`) rises to 16 MiB. Touches:
route-admission validation (`validate_policy` — per-route signed caps may still
pin lower), `validate_proxy_request_head`, and the door's
`DefaultBodyLimit`. Request bodies stay BUFFERED (a `claude` request is one
JSON blob; 16 MiB buffered is fine, and the Hello-then-raw-body wire already
carries `body_len`). This is a consensus-admission change in the gateway
module → simnode coverage for the new bound (accept ≤ 16 MiB policy, reject
above), scoped to the gateway scenarios target while `governance_scenarios`
is red from the concurrent Join-v2 campaign.

## C. Airlock body AEAD (broker ↔ enclave, stateless)

Threat closed: the publisher node's HOST process relays `/v1/*` with plaintext
bodies and a reusable bearer in `Authorization`. After this change it sees
ciphertext and holds a token it cannot use.

- **Key material, no new enclave state:** the session token's signed claims
  gain the client's ephemeral X25519 public key from the handshake. Per
  request the enclave re-derives `secret = DH(seal_sk, eph_pk)` and
  `body_key = HKDF(secret, "airlock-body-v1")`; the broker holds the same
  secret from `open_session`. Tokens remain enclave-signed and stateless.
- **Request:** broker seals the whole JSON body (existing `aead` module
  primitives, ChaCha20-Poly1305, random nonce prefix) and marks the request
  (`x-airlock-body-seal: v1`). A session opened for sealed mode REFUSES
  unsealed bodies — a stolen bearer alone is useless.
- **Response:** the enclave seals the upstream stream as length-prefixed
  sealed chunks (`[u32 len][ct]`; nonce = per-response random prefix ‖ u64
  chunk counter, so a reordered, replayed, or dropped-then-resumed chunk fails
  to open; a first sealed head-chunk carries the inner content-type); the
  broker unframes, unseals in order, and forwards plain SSE to the unmodified
  sandbox. Chunking composes with (A)
  — sealed chunks ride `BodyChunk` frames untouched by the gateway.
- Both airlock consumers (capability-host broker, `airlock-broker`) speak v1;
  `airlock-cli run` too. Flag-day, in-place per mandate.

## Testing (this box, no TEE/WG needed)

- Frame-path streaming: single-node overlay e2e over the unified duplex path —
  a long SSE body (> 4 MiB, > old clamp) streams through gateway plane frames
  end to end; running-cap abort mid-stream; `Failure` mid-stream surfaces as
  truncation.
- Simnode: gateway admission bound for 16 MiB.
- AEAD: unit (roundtrip, tamper, chunk-reorder rejection via counter/nonce
  design, wrong-key), capability-host in-process custody e2e with sealing on
  (upstream must receive plaintext exactly once — at the enclave), node e2e
  combined streaming+AEAD.
- Deliverable shape: two stacked PRs — (1) A+B gateway streaming + ceiling,
  (2) C airlock AEAD.

## Out of scope

Request-body streaming (buffered blobs suffice), session revocation endpoint
(restart revokes; revisit with multi-tenancy), per-sub budget config, WS-lane
changes, hardware items (task #26).
