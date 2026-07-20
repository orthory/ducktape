# SSE-over-overlay Streaming + Airlock Body AEAD Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stream proxied LoopbackHttp responses end-to-end through the gateway overlay (PR1, with the 16 MiB request-ceiling raise), then encrypt airlock bodies broker↔enclave so no node host on the path reads conversation content (PR2).

**Architecture:** PR1 removes the three re-buffering points around the ALREADY-chunked `ResponseHead`/`BodyChunk`/`End` frame wire: `proxy_loopback` forwards `bytes_stream()` chunks through a bounded channel, the server half pumps chunks to frames, the client half pumps frames back to a channel, and the browser door serves `Body::from_stream`. Self-serve (publisher == self) unifies onto the same frame protocol over `tokio::io::duplex` (the WS lane's existing pattern), so the single-node e2e exercises the real wire. PR2 derives a body key from the existing handshake ECDH (new HKDF label), carries the client ephemeral pk in the token claims so the enclave stays stateless, seals request bodies as one blob and response streams as counter-nonce sealed chunks with an authenticated final marker.

**Tech Stack:** existing gateway `ProxyFrame` codec (unchanged), tokio mpsc/duplex, reqwest `bytes_stream`, axum `Body::from_stream`, airlock `aead` module (ChaCha20-Poly1305 + HKDF-SHA256).

**Spec:** `docs/superpowers/specs/2026-07-20-sse-overlay-streaming-body-aead-design.md`

## Global Constraints

- Worktree `/home/eddy/dev/ducktape/.worktree/sse-overlay-streaming`, branch `feat-sse-overlay-streaming`, PR1 vs `dev`; PR2 stacked (`feat-airlock-body-aead` off PR1's branch, retarget to dev after PR1 merges — remember the `--delete-branch` auto-close trap: never delete PR1's branch while PR2 is open on it).
- Every cargo invocation: `CARGO_INCREMENTAL=0 RUSTC_WRAPPER="" RUST_MIN_STACK=134217728` (this box's rustc-segfault recipe; retry once on SIGSEGV).
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`. Workspace FORBIDS clap. Logging via `tracing` in node code; `eprintln!` only in standalone bins. Never log URI paths or key material.
- Wire/protocol changes are IN-PLACE (no-backcompat mandate); both nodes of a network ship together.
- Simnode is currently RED in `governance_scenarios` (Join-v2 campaign) — run gateway coverage via `cargo test -p simnode --test gateway_scenarios` (or the file that holds gateway admission scenarios; locate with `rg -l "validate_policy|max_request_bytes" crates/sim/simnode/tests/`), never the full `-p simnode` gate, and say so in the PR.
- Key current-code facts (verified): `GatewayJob::Http{publisher_node, max_response_bytes, head, body, reply: oneshot<Result<GatewayResponse, GatewayFailure>>}` and `GatewayResponse{head, body: Vec<u8>}` in `bin/noded/src/gateway_http.rs:34-57`; client job loop + `PROXY_IO_TIMEOUT` wrap at `bin/node/src/gateway_plane.rs:100-132`; `proxy_remote` :263, `serve_current` :335, `proxy_loopback` :516 (reqwest client has a TOTAL `.timeout(PROXY_IO_TIMEOUT)` at :536 — must become connect/head-scoped), buffered collect at :645-656; server-half accept :199-258; `write_frame`/`read_frame` :1189-1231; `write_proxy_response` :1235, `read_proxy_response` :1259 (0-cap clamp at :1270-1274); WS duplex self-serve pattern :141-158; browser door buffers at `bin/noded/src/gateway_http.rs` (`Body::from(response.body)`); JSON lane `GatewayProxyReply.body_b64` stays buffered by contract; `MAX_REQUEST_BODY_BYTES` (1 MiB) + `validate_policy` in `crates/networking/gateway/src/interface.rs`, head bound in `proxy.rs`; airlock `client_handshake`/`enclave_session_key` + `aead::{hkdf32, seal, open}` + token `Claims{sub, iat, exp, max_requests}`.

---

# PR1 — gateway streaming + 16 MiB request ceiling

### Task 1: streaming response types + client-side frame pump

**Files:**
- Modify: `bin/noded/src/gateway_http.rs` (types only, :34-57)
- Modify: `bin/node/src/gateway_plane.rs` (`read_proxy_response` region :1259)
- Test: `bin/node/src/gateway_plane.rs` `#[cfg(test)]` (duplex-driven unit tests)

**Interfaces (produced):**
- `pub type GatewayBody = tokio::sync::mpsc::Receiver<Result<bytes::Bytes, GatewayFailure>>;` (noded; `bytes` is already in the tree via axum/reqwest — add `bytes = { workspace = true }` to noded if not a direct dep).
- `GatewayResponse` becomes `pub struct GatewayResponse { pub head: gateway::ProxyResponseHead, pub body: GatewayBody }` (drop `Clone`/`PartialEq` derives; tests compare heads and collected bodies).
- In gateway_plane: `async fn read_proxy_head<S>(stream: &mut S, buf: &mut Vec<u8>) -> Result<gateway::ProxyResponseHead, GatewayFailure>` and `fn spawn_body_pump<S: AsyncRead + Unpin + Send + 'static>(stream: S, buf: Vec<u8>, max_response_bytes: u64) -> GatewayBody` — pump reads `BodyChunk` frames, enforces the RUNNING cap (`0` = unbounded; the old `MAX_PROXY_FRAME_BYTES` clamp for `0` is DELETED), forwards `Failure` as an `Err` item, ends the channel on `End`.

- [ ] **Step 1: failing unit test** in `gateway_plane.rs` tests mod — drive the new reader against a hand-written frame byte stream over `tokio::io::duplex`:

```rust
#[tokio::test]
async fn streamed_response_arrives_chunk_by_chunk_and_zero_cap_is_unbounded() {
    let (mut writer, mut reader) = tokio::io::duplex(64 * 1024);
    let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
    let big = vec![0xABu8; 5 * 1024 * 1024]; // > the old 4 MiB clamp
    let frames = {
        let mut out = Vec::new();
        out.extend(gateway::encode_frame(&gateway::ProxyFrame::ResponseHead(head.clone())).unwrap());
        for chunk in big.chunks(gateway::MAX_CHUNK_BYTES) {
            out.extend(gateway::encode_frame(&gateway::ProxyFrame::BodyChunk(chunk.to_vec())).unwrap());
        }
        out.extend(gateway::encode_frame(&gateway::ProxyFrame::End).unwrap());
        out
    };
    tokio::spawn(async move { writer.write_all(&frames).await.unwrap() });

    let mut buf = Vec::new();
    let got_head = read_proxy_head(&mut reader, &mut buf).await.unwrap();
    assert_eq!(got_head.status, 200);
    let mut body = spawn_body_pump(reader, buf, 0);
    let mut total = Vec::new();
    while let Some(chunk) = body.recv().await {
        total.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(total, big, "a 5 MiB body must stream through with cap 0");
}

#[tokio::test]
async fn running_cap_aborts_mid_stream() {
    let (mut writer, mut reader) = tokio::io::duplex(64 * 1024);
    let head = gateway::ProxyResponseHead { status: 200, headers: vec![] };
    let mut frames = gateway::encode_frame(&gateway::ProxyFrame::ResponseHead(head)).unwrap();
    for _ in 0..4 {
        frames.extend(gateway::encode_frame(&gateway::ProxyFrame::BodyChunk(vec![0u8; 1024])).unwrap());
    }
    frames.extend(gateway::encode_frame(&gateway::ProxyFrame::End).unwrap());
    tokio::spawn(async move { writer.write_all(&frames).await.unwrap() });

    let mut buf = Vec::new();
    read_proxy_head(&mut reader, &mut buf).await.unwrap();
    let mut body = spawn_body_pump(reader, buf, 2048);
    let mut seen = 0usize;
    let mut aborted = false;
    while let Some(item) = body.recv().await {
        match item {
            Ok(chunk) => seen += chunk.len(),
            Err(_) => { aborted = true; break; }
        }
    }
    assert!(aborted, "exceeding the running cap must surface an error item");
    assert!(seen <= 2048);
}
```

- [ ] **Step 2:** run `cargo test -p node-bin --lib gateway_plane` → FAIL (functions missing).
- [ ] **Step 3: implement.** `read_proxy_head` = the head-arm of today's `read_proxy_response` (ResponseHead → validate + return; Failure → Err; else Err). `spawn_body_pump`:

```rust
/// Frame → chunk pump for a streamed response body. Runs until `End`,
/// `Failure`, overflow of the RUNNING cap (`0` = unbounded), or the receiver
/// hanging up. Backpressure is the bounded channel + the paced stream.
fn spawn_body_pump<S: AsyncRead + Unpin + Send + 'static>(
    mut stream: S,
    mut buf: Vec<u8>,
    max_response_bytes: u64,
) -> GatewayBody {
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, GatewayFailure>>(16);
    tokio::spawn(async move {
        let mut total: u64 = 0;
        loop {
            let frame = match read_frame(&mut stream, &mut buf).await {
                Ok(frame) => frame,
                Err(failure) => {
                    let _ = tx.send(Err(failure)).await;
                    return;
                }
            };
            match frame {
                gateway::ProxyFrame::BodyChunk(chunk) => {
                    total += chunk.len() as u64;
                    let over_cap = max_response_bytes != 0 && total > max_response_bytes;
                    if over_cap {
                        let _ = tx
                            .send(Err(GatewayFailure::Unavailable(
                                "publisher exceeded the response cap".into(),
                            )))
                            .await;
                        return;
                    }
                    if tx.send(Ok(bytes::Bytes::from(chunk))).await.is_err() {
                        return; // caller went away; drop the stream
                    }
                }
                gateway::ProxyFrame::End => return,
                gateway::ProxyFrame::Failure(failure) => {
                    let _ = tx.send(Err(failure_from(failure))).await;
                    return;
                }
                _ => {
                    let _ = tx
                        .send(Err(GatewayFailure::Unavailable(
                            "unexpected frame in gateway response body".into(),
                        )))
                        .await;
                    return;
                }
            }
        }
    });
    rx
}
```

Delete `read_proxy_response`. `GatewayResponse` reshaped in noded (compile errors elsewhere are Tasks 2-5's job — for THIS task's commit, patch call sites minimally to collect the stream where they buffered before, so the tree compiles at every commit: a small helper `async fn collect_body(body: &mut GatewayBody) -> Result<Vec<u8>, GatewayFailure>` in noded used by the JSON lane and (temporarily) the browser door).
- [ ] **Step 4:** tests pass; `cargo build -p node-bin -p noded` green. Commit `feat(gateway): streamed response body type + client frame pump`.

### Task 2: publisher side streams — `serve_current`/`proxy_loopback` return head + body stream

**Files:**
- Modify: `bin/node/src/gateway_plane.rs` (`serve_current` :335, `proxy_loopback` :516, `serve_duckfs` arm)

**Interfaces:**
- Consumes: `GatewayBody`, `GatewayResponse{head, body}` (Task 1).
- Produces: `serve_current(..., head, body) -> Result<GatewayResponse, GatewayFailure>` (same signature, but the returned `body` is now the live stream). `proxy_loopback` sends the head as soon as upstream headers validate, then forwards chunks through the channel with the running cap.

- [ ] **Step 1:** rewrite `proxy_loopback`'s tail (from the reqwest send onward):
  - Client builder: replace `.timeout(PROXY_IO_TIMEOUT)` with `.connect_timeout(PROXY_IO_TIMEOUT)` — the TOTAL timeout would kill long SSE bodies; the head is still deadline-bound because `serve_current` is called inside the server-half `PROXY_IO_TIMEOUT` (Task 3 narrows that wrap to end at head-return).
  - Keep the `content_length` pre-check and header assembly EXACTLY as-is, but treat `max_response_bytes == 0` as unbounded in the pre-check: `let capped = route.policy.max_response_bytes != 0; if capped && response.content_length().is_some_and(|l| l > route.policy.max_response_bytes) { ... }`.
  - Replace the buffered collect loop (:645-656) with:

```rust
    let cap = route.policy.max_response_bytes;
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<bytes::Bytes, GatewayFailure>>(16);
    let is_head = head.method == gateway::RouteMethod::Head;
    tokio::spawn(async move {
        if is_head {
            return; // HEAD: headers only, upstream body dropped
        }
        let mut chunks = response.bytes_stream();
        let mut total: u64 = 0;
        while let Some(chunk) = chunks.next().await {
            let item = match chunk {
                Ok(chunk) => {
                    total += chunk.len() as u64;
                    let over_cap = cap != 0 && total > cap;
                    if over_cap {
                        let _ = tx
                            .send(Err(GatewayFailure::Unavailable(
                                "loopback response exceeds the signed route cap".into(),
                            )))
                            .await;
                        return;
                    }
                    Ok(chunk)
                }
                Err(error) => {
                    let _ = tx.send(Err(GatewayFailure::Unavailable(error.to_string()))).await;
                    return;
                }
            };
            if tx.send(item).await.is_err() {
                return;
            }
        }
    });
    Ok(GatewayResponse { head: response_head, body: rx })
```

  - `serve_duckfs`: keep its buffered `Vec<u8>` internals; wrap the result: `let (tx, rx) = tokio::sync::mpsc::channel(1); ... tx.try_send(Ok(bytes::Bytes::from(body))).ok(); Ok(GatewayResponse { head, body: rx })` (channel of capacity 1 holds the single chunk; sender dropped ⇒ End). If a duckfs body can exceed `MAX_CHUNK_BYTES`... it can (files up to the route cap) — the SERVER half re-chunks at frame-write time (Task 3 splits any `Bytes` > `MAX_CHUNK_BYTES`), so one big chunk here is fine.
- [ ] **Step 2:** `cargo build -p node-bin` — remaining compile errors are only in the server half / self-serve call sites touched next; if any, stub-collect there temporarily with Task 1's `collect_body`. Commit `feat(gateway): publisher proxy_loopback streams the upstream body`.

### Task 3: server half writes streaming frames

**Files:**
- Modify: `bin/node/src/gateway_plane.rs` (accept-loop body :235-257, `write_proxy_response` :1235)

**Interfaces:**
- Produces: `async fn write_proxy_response<S: AsyncWrite + Unpin>(stream: &mut S, outcome: Result<GatewayResponse, GatewayFailure>) -> std::io::Result<()>` — SAME name/signature, now drains the body channel: `ResponseHead`, then for each `Ok(bytes)` item one-or-more `BodyChunk` frames (split at `gateway::MAX_CHUNK_BYTES`) each followed by `flush` (SSE latency), then `End`; an `Err(failure)` item mid-body emits `Failure` and stops; pre-head failure stays a single `Failure` frame.
- The server-half timeout NARROWS: `PROXY_IO_TIMEOUT` wraps body-read + `serve_current` (up to and including obtaining the head), while `write_proxy_response`'s body drain runs UNWRAPPED (WS-lane precedent — a long SSE body has no overall deadline; the paced stream + peer hangup bound it).

- [ ] **Step 1:** implement:

```rust
async fn write_proxy_response<S: AsyncWrite + Unpin>(
    stream: &mut S,
    outcome: Result<GatewayResponse, GatewayFailure>,
) -> std::io::Result<()> {
    match outcome {
        Ok(mut response) => {
            if let Err(error) = gateway::validate_response_head(&response.head) {
                write_frame(stream, &failure_frame(&GatewayFailure::Unavailable(error))).await?;
                return stream.flush().await;
            }
            write_frame(stream, &gateway::ProxyFrame::ResponseHead(response.head)).await?;
            stream.flush().await?;
            while let Some(item) = response.body.recv().await {
                match item {
                    Ok(chunk) => {
                        for piece in chunk.chunks(gateway::MAX_CHUNK_BYTES) {
                            write_frame(stream, &gateway::ProxyFrame::BodyChunk(piece.to_vec()))
                                .await?;
                        }
                        stream.flush().await?;
                    }
                    Err(failure) => {
                        write_frame(stream, &failure_frame(&failure)).await?;
                        return stream.flush().await;
                    }
                }
            }
            write_frame(stream, &gateway::ProxyFrame::End).await?;
        }
        Err(failure) => write_frame(stream, &failure_frame(&failure)).await?,
    }
    stream.flush().await
}
```

- [ ] **Step 2:** in the accept loop, move `write_proxy_response` OUT of the `PROXY_IO_TIMEOUT` wrap (it currently sits inside a second timeout at :253-257 — delete that wrap; the timeout now covers only body-read + `serve_current`). Same change on the client side: `proxy_remote`'s timeout covers open + body write + `read_proxy_head`; the body pump lives beyond it (it's a spawned task from Task 1). The `GatewayJob::Http` handler's outer `PROXY_IO_TIMEOUT` (:108) also narrows: it wraps everything up to the reply-send of the head-carrying `GatewayResponse` — which is exactly what the existing wrap does once `serve_current`/`proxy_remote` return at head-time; leave the wrap in place, it no longer covers the body by construction.
- [ ] **Step 3:** `cargo build -p node-bin` green; unit tests from Task 1 still green. Commit `feat(gateway): server half streams body frames; body freed from the one-shot deadline`.

### Task 4: self-serve unification over duplex

**Files:**
- Modify: `bin/node/src/gateway_plane.rs` (client Http job arm :109-113, server accept-loop body)

**Interfaces:**
- Produces: `async fn serve_proxy_stream<S: AsyncRead + AsyncWrite + Unpin + Send + 'static>(commands, workspace, own_node, caller_node, head, stream)` — the extracted per-connection server body (read `body_len`, `serve_current`, `write_proxy_response`), used by BOTH the accept loop and the self-serve arm.

- [ ] **Step 1:** extract the accept-loop per-connection body (:235-258, post-`head.upgrade` branch) into `serve_proxy_stream`; the accept loop calls it.
- [ ] **Step 2:** the self-serve arm replaces its direct `serve_current` call with the WS lane's duplex pattern:

```rust
if publisher_node == own_node {
    let (server_end, mut caller_end) = tokio::io::duplex(64 * 1024);
    let serve_commands = commands.clone();
    let serve_workspace = workspace.clone();
    let head_for_server = head.clone();
    let body_for_server = body.clone();
    tokio::spawn(async move {
        // Self-serve rides the SAME frame protocol as the overlay path, so
        // every e2e on a single node exercises the real wire.
        serve_proxy_stream(
            &serve_commands,
            &serve_workspace,
            &own_node,
            &own_node,
            &head_for_server,
            &body_for_server,
            server_end,
        )
        .await;
    });
    // Client side: same read path as remote, minus the overlay open.
    read_streamed_response(&mut caller_end, max_response_bytes).await
} else { ... proxy_remote ... }
```

  where `read_streamed_response(stream, cap)` = `read_proxy_head` + `spawn_body_pump` composed (also used by `proxy_remote` — extract it there first so remote and self-serve share it). NOTE: `serve_proxy_stream` for self-serve takes the body directly (no `read_exact` needed) — give it the signature `(commands, workspace, own_node, caller_node, head, body: Option<&[u8]>, stream)`: `Some(body)` = self-serve (skip the stream read), `None` = overlay (read `body_len` from the stream). Keep it ONE function so the serve/write halves cannot drift.
- [ ] **Step 3:** run the airlock single-node e2e (it self-serves, so it now crosses the frame wire): `cargo test -p node-bin --test airlock_gateway_e2e airlock_single_node_self_serves_its_own_route` → PASS. Commit `feat(gateway): self-serve rides the frame wire (one proxy path)`.

### Task 5: doors — browser door streams, JSON lane stays buffered

**Files:**
- Modify: `bin/noded/src/gateway_http.rs` (`gateway_browser_proxy` tail; `gateway_proxy` JSON lane)

- [ ] **Step 1:** browser door: replace `Body::from(response.body)` with

```rust
    let body = if matches!(status_class, HeadlessClass::NoBody) {
        Body::empty() // keep the existing HEAD/204/304 handling exactly
    } else {
        Body::from_stream(tokio_stream::wrappers::ReceiverStream::new(response.body).map(
            |item| item.map_err(|failure| std::io::Error::other(format!("{failure:?}"))),
        ))
    };
```

  (match the door's real local names — the excerpt names are from the current buffered tail; `tokio-stream` is already in the workspace via axum, add to noded's Cargo if not direct). A mid-stream `Err` aborts the HTTP response — the truncation contract from the spec.
- [ ] **Step 2:** JSON lane `gateway_proxy`: buffered by contract — call `collect_body(&mut response.body).await?` (Task 1 helper) before base64. The old whole-body cap re-check in `proxy_current` (`response.body.len() > max_response_bytes`) is DELETED — the cap now lives in the pumps (running).
- [ ] **Step 3:** `cargo build -p noded -p node-bin && cargo test -p noded` green. Commit `feat(gateway): browser door streams; JSON lane collects by contract`.

### Task 6: 16 MiB request ceiling + simnode admission coverage

**Files:**
- Modify: `crates/networking/gateway/src/interface.rs` (`MAX_REQUEST_BODY_BYTES`), `crates/networking/gateway/src/proxy.rs` (uses the const — verify no independent literal), `bin/noded/src/gateway_http.rs` (`DefaultBodyLimit`)
- Test: the simnode gateway scenarios file (locate: `rg -l "max_request_bytes" crates/sim/simnode/tests/`) + gateway crate unit tests

- [ ] **Step 1:** `pub const MAX_REQUEST_BODY_BYTES: u64 = 16 * 1024 * 1024;` (from 1 MiB; keep the doc comment honest about WHY: a claude turn's context is multi-MB). Grep for any duplicated `1024 * 1024` literal in gateway validation and the noded body limit — all must reference the const.
- [ ] **Step 2:** gateway crate unit test (interface.rs tests mod): `validate_policy` accepts `max_request_bytes = 16 MiB`, rejects `16 MiB + 1`.
- [ ] **Step 3:** simnode: extend the existing gateway route-admission scenario with a 16 MiB-policy SetRoute accepted + over-ceiling rejected. Run ONLY the gateway scenarios target. Commit `feat(gateway)!: request-body ceiling 1 MiB -> 16 MiB (admission + head + door)`.

### Task 7: streaming e2e + gates + PR1

**Files:**
- Modify: `bin/node/tests/airlock_gateway_e2e.rs` (or a new `gateway_streaming_e2e.rs` reusing `common`)
- Modify: `crates/modules/system/airlock/README.md` + exec/auth spec §graft "Remaining" (streaming shipped)

- [ ] **Step 1:** single-node streaming e2e (self-serve now == real wire): publish a LoopbackHttp route with `max_response_bytes: 0`, upstream = axum handler that sends an SSE body of 6 MiB in 64 KiB chunks through a channel-backed `Body::from_stream`; client calls through the browser door and asserts (a) total bytes == 6 MiB (beats the old 4 MiB clamp), (b) the FIRST bytes arrive before the upstream finishes sending (event-driven: upstream holds a `tokio::sync::Notify` it releases only after the client has read the first chunk — no sleeps). Second test: route with `max_response_bytes: 64 KiB`, upstream streams 1 MiB → client sees truncation/abort, publisher does not wedge.
- [ ] **Step 2:** full gates: clippy (`gateway`, `noded`, `node-bin`), `cargo test -p node-bin --test airlock_gateway_e2e`, the new streaming e2e, gateway crate tests, simnode gateway target, workspace build, `cargo check -p files --no-default-features`.
- [ ] **Step 3:** PR1 vs dev: title `feat(gateway): stream LoopbackHttp responses over the overlay; 16 MiB request ceiling`. Body: the three deleted buffer points, self-serve unification (test win), timeout semantics change (body freed from one-shot deadline), `0`-cap now truly unbounded, ceiling change + simnode note (scoped target while governance_scenarios is red). Adversarial review before merge; then merge on green per session flow.

---

# PR2 — airlock body AEAD (stacked on PR1)

### Task 8: body keys in handshake + claims

**Files:**
- Modify: `crates/modules/system/airlock/src/handshake.rs`, `src/token.rs`, `src/wire.rs` (SessionRequest), `src/server.rs` (/session), `src/client.rs` (open_session)

**Interfaces (produced):**
- `handshake.rs`: `pub struct SessionKeys { pub session: [u8; 32], pub body: [u8; 32] }`; `client_handshake(seal_pk) -> ([u8; 32], SessionKeys)` (second HKDF label `b"airlock-body-v1"` over the SAME shared secret); `enclave_session_keys(seal_kp, client_eph_pk) -> SessionKeys` (replaces `enclave_session_key`).
- `token.rs`: `Claims` gains `pub eph: String` (base64url of the client ephemeral pk) and `pub seal: bool` — REQUIRED fields, flag-day.
- `wire.rs`: `SessionRequest` gains `pub body_seal: bool`; server copies it into claims; client `open_session` gains a variant `open_session_sealed(seal_pk, sub) -> Result<(String, SessionKeys)>` returning the token AND the keys (existing `open_session` keeps its signature for the cli's non-sealed path and sets `body_seal: false`).

- [ ] **Step 1:** failing tests: handshake both-sides-agree extended to body key; token round-trip with new fields; server /session e2e asserts claims carry `eph` == the handshake pk and `seal` mirrors the request (extend `crates/modules/system/airlock/tests/e2e.rs` custody test).
- [ ] **Step 2:** implement; all airlock tests green (`--features server,client,verify,testkit`). Commit `feat(airlock): body-key derivation + eph/seal claims (stateless enclave rederivation)`.

### Task 9: `bodyseal` module — sealed request blob + sealed response chunk stream

**Files:**
- Create: `crates/modules/system/airlock/src/bodyseal.rs` (+ `pub mod bodyseal;` in lib.rs)
- Test: in-file unit tests

**Interfaces (produced):**
- `pub const SEAL_HEADER: &str = "x-airlock-body-seal";` / `pub const SEAL_V1: &str = "v1";`
- Request: `pub fn seal_request(keys: &SessionKeys, body: &[u8]) -> Vec<u8>` / `pub fn open_request(keys: &SessionKeys, blob: &[u8]) -> Result<Vec<u8>>` — `aead::seal/open` under `hkdf32(&keys.body, b"airlock-body-req-v1")`.
- Response stream: `pub struct StreamSealer` / `pub struct StreamOpener`:
  - wire: `[16B stream salt]` once, then repeated `[u32 BE len][ct]`.
  - per-stream key `= hkdf32_salted(&keys.body, salt, b"airlock-body-stream-v1")` (add `aead::hkdf32_salted(shared, salt, label)` — `Hkdf::new(Some(salt), shared)`).
  - nonce = 12 bytes: `[4B zero ‖ u64 BE counter]`, counter from 0 — order/replay/reorder authenticated by position.
  - plaintext framing: first sealed chunk is `[0x02] ‖ inner_content_type_utf8` (head marker); data chunks `[0x00] ‖ data`; final chunk `[0x01]` alone — authenticated EOF so truncation is detectable.
  - `StreamSealer::new(keys) -> (Self, Vec<u8> /* salt prefix to emit */)`, `.seal_head(content_type) -> Vec<u8>`, `.seal_chunk(&[u8]) -> Vec<u8>`, `.seal_final() -> Vec<u8>` (each returns `[len][ct]` framed bytes).
  - `StreamOpener::new(keys)`, `.feed(&mut self, bytes: &[u8]) -> Result<Vec<OpenedItem>>` incremental parser (`OpenedItem::{Head(String), Data(Vec<u8>), Final}`); errors on wrong order, bad tag, counter mismatch, data-after-final.

- [ ] **Step 1:** failing unit tests: round-trip (head + N chunks + final across arbitrary feed splits), tamper rejection, REORDER rejection (swap two sealed chunks → opener errors), truncation detection (missing final → opener never yields Final; caller treats as truncated), wrong-key rejection.
- [ ] **Step 2:** implement; tests green. Commit `feat(airlock): bodyseal — sealed request blob + counter-nonce sealed response stream`.

### Task 10: enclave enforces + seals

**Files:**
- Modify: `crates/modules/system/airlock/src/server.rs` (the `/v1/{*rest}` proxy handler)

**Interfaces:**
- Consumes: `Claims{eph, seal}`, `enclave_session_keys`, `bodyseal::*`.
- Behavior: verify token (existing); derive `keys = enclave_session_keys(&st.seal_kp, &decode(claims.eph))` per request. If `claims.seal`: REQUIRE `SEAL_HEADER == v1` and open the request body (`open_request`) before forwarding upstream — an unsealed body on a sealed session is 400 `airlock: sealed session requires a sealed body` (the stolen-bearer defense); the response is re-sealed: emit salt prefix + sealed head-chunk (upstream content-type) + sealed data chunks per upstream chunk + sealed final, outer `content-type: application/octet-stream`. If `!claims.seal`: exactly today's plaintext passthrough.

- [ ] **Step 1:** failing airlock e2e tests (tests/e2e.rs): (a) sealed custody path — client opens `open_session_sealed`, seals the request, unseals the streamed response, sees the upstream's SSE plaintext; the MOCK UPSTREAM asserts it received the PLAINTEXT JSON (proving unseal happened at the enclave, nowhere else); (b) a sealed session refuses an unsealed body (400); (c) a plaintext session behaves exactly as before (existing tests keep passing).
- [ ] **Step 2:** implement over the axum handler's streaming body (PR1's server proxies via reqwest `bytes_stream` — wrap it with `StreamSealer`). Tests green. Commit `feat(airlock): enclave seals/unseals bodies; sealed sessions refuse plaintext`.

### Task 11: broker sides + combined e2e + PR2

**Files:**
- Modify: `crates/modules/system/capability-host/src/broker.rs` (airlock arm: `open_session_sealed`, seal outgoing `/v1` bodies, unseal the response stream before the sandbox), `bin/airlock-broker/src/main.rs` (same), `bin/airlock-cli/src/main.rs` (`run` uses sealed mode)
- Modify: `crates/modules/system/airlock/README.md` + exec/auth spec (AEAD shipped; revocation = restart documented)

- [ ] **Step 1:** capability-host: the airlock `AnthropicAuth::Airlock` arm stores `SessionKeys`; `send_upstream` seals the body + sets `SEAL_HEADER`; the response path wraps the streamed body through `StreamOpener`, forwarding plaintext SSE chunks to the sandbox as they open (the broker already streams via `Body::from_stream`). Re-handshake on 401 refreshes keys+token together.
- [ ] **Step 2:** failing test in capability-host: extend the in-process airlock custody test — recording upstream between broker and gateway is not possible (direct), so assert at the GATEWAY-side: run the broker against the in-process gateway and a recording MOCK ANTHROPIC upstream; assert (a) sandbox receives plaintext SSE, (b) the mock upstream received plaintext exactly once, (c) a tap on the broker→gateway HTTP request (the recording_gateway helper pattern) sees `SEAL_HEADER: v1` and a body that does NOT contain the plaintext marker bytes.
- [ ] **Step 3:** node combined e2e: the single-node airlock route test gains a sealed variant — sealed session through the overlay frame wire, plaintext only at the two ends.
- [ ] **Step 4:** gates (airlock, capability-host, airlock-broker, airlock-cli, node-bin clippy+tests; workspace build) → PR2 `feat(airlock): body AEAD broker<->enclave — path hosts see ciphertext; stolen bearers are useless`, stacked on PR1's branch, retargeted + merged after PR1 (adversarial review first; never `--delete-branch` PR1 while PR2 is open).
