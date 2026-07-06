# Video Calls in Chat ("concall") — Design

Date: 2026-07-06 · Branch: `feat/video-call-design` (from `dev`)

Voice + video conference calls as a feature of the **chat module**, with media
riding the **data-plane** over the WireGuard overlay and **WebRTC** as the
local capture/encode/render leg. Extends the existing voice engine
(`crates/apps/chat/src/voice/`) rather than adding anything beside it.

## Ground truth this design stands on

- **Data-plane** (`crates/system/data-plane/`): off-consensus byte transport
  designed to ride the WG overlay (`dt-*`, fd::/48 ULA, per-peer /128).
  Datagram class (latency-first, drop-oldest, `MAX_DATAGRAM = 1372`) + stream
  class (paced bulk). `Service { StateSync = 1, Voice = 2 }` is an append-only
  wire-stable registry. Admission is default-deny via `AdmissionPolicy`
  `(peer, service, flow)`, derived from finalized consensus state. Identity is
  the transport's `PeerId` (WG cryptokey routing) — no SSRC, nothing to spoof.
  **Only the sim transport exists**; the real overlay-socket arm is unbuilt
  and no production binary constructs a `DataPlane` yet.
- **Voice engine** (`crates/apps/chat/src/voice/`): Opus/48k/mono/20 ms over
  `Service::Voice` datagrams behind an 8-byte media header; per-speaker jitter
  buffer + client-side mix. Sim-proven; no capture/playback/AEC wired.
- **Chat module** (`crates/apps/chat/`): channels, membership
  (`SetMembership`, `PostPolicy::MembersOnly`), authors are origin-bound
  (`AuthorRef::User(pubkey)` → profiles display names). Its docs already say
  channel membership will drive the voice engine's `AdmissionPolicy`.
- **Module-set invariant**: `global_root` hashes the module count and every
  `(id, root)` — a **new module id is consensus-breaking**. New op variants in
  an existing module are also divergent for old binaries, so call ops must be
  **protocol-version-gated** via the upgrade module (`Env::protocol_version`),
  the established no-downtime rollout path.
- **NAT layer**: coordinator is STUN-rendezvous only (bespoke protocol, not
  RFC 5389 — a browser ICE agent cannot use it); failed punch is terminal; no
  relay. The punched socket belongs to `NatClient`; the shareable substrate is
  the WG tunnel itself. **No STUN/TURN/WebRTC code exists in the repo.**
- **Webview WebRTC reality (researched 2026-07-06)**:
  - Linux WebKitGTK: `RTCPeerConnection` is **compiled out** of stock distro
    builds (`ENABLE_WEB_RTC` off outside experimental builds; Debian 2.53.4
    verified). Custom WebKitGTK is not shippable; even then it's a ~55%-WPT
    stack. `ENABLE_MEDIA_STREAM` (getUserMedia) is compiled in but runtime-off
    and wry never enables it. **In-webview calls on Linux are off the table.**
  - macOS WKWebView: works (same engine as Safari). Needs
    `NSCameraUsageDescription`/`NSMicrophoneUsageDescription` in a custom
    `Info.plist`, plus `com.apple.security.device.camera`/`.audio-input`
    entitlements for hardened-runtime builds. wry's UI delegate defaults media
    capture permission to Grant, so only the OS TCC prompt shows; wry's
    `with_permission_handler` (merged 2026-06-11, unreleased) gives control.
  - Windows WebView2: Chromium, works; handle `PermissionRequested` and the
    persisted-Block gotcha.
- Precedent: Hopp (Tauri pair-programming app) ships native Rust media +
  webview UI because of exactly the WebKitGTK blocker.

## Goals

- Start/join/leave a voice+video call attached to a chat channel ("huddle"
  model): roster in the channel header, tile grid, mute/camera toggles.
- Media never touches consensus; call membership does (it is the admission
  source and the durable "a call is happening" signal).
- No new NAT machinery: media flows only between peers that already hold a WG
  tunnel. No STUN/TURN/ICE on the wire, no third-party relay, no external SFU.
- Huddle scale: ≤ 8 participants, full mesh at the node level.

## Non-goals (v1)

- Screenshare (`getDisplayMedia`), recording (blob-plane manifest pattern),
  agent/headless participants (the native voice-engine path keeps this door
  open), simulcast/SVC, media E2EE beyond WG (see Private-messaging note),
  PSTN/external guests, mobile.

## Approaches considered

**A. Browser-P2P WebRTC mesh** (classic: RTCPeerConnection per pair, SDP/ICE
signaled through consensus ops, STUN/TURN infra). Rejected: dead on Linux
(no RTCPeerConnection), needs the TURN relay we just deliberately removed,
duplicates NAT traversal the WG overlay already solved, bypasses the
data-plane and its admission model entirely.

**B. Node media gateway + data-plane backbone — CHOSEN.** The webview holds
exactly one `RTCPeerConnection`, to **its own local node** over
`127.0.0.1` (host candidates only — no STUN, no trickle, no mDNS problem;
browsers reveal real host candidates once getUserMedia is granted, and the
node's answer candidate is `127.0.0.1`). The node terminates WebRTC
(DTLS/SRTP/RTCP) with **str0m** (sans-IO, server-grade, actively maintained;
fits the daemon's single-threaded actor style better than tokio-coupled
webrtc-rs), then repacketizes encoded frames onto data-plane datagrams to
every other participant's node. **SDP never crosses the network**: cross-node
"signaling" is just consensus call-roster ops + handshake-free
`FlowId::derive`. The browser contributes what it is uniquely good at —
capture, hardware encode/decode, AEC/AGC, rendering — and the data-plane
contributes transport, identity, and admission.

**C. Fully-native media engine** (GStreamer capture/encode + native render
window, no WebRTC anywhere — Hopp's end state). Best latency and one pipeline
for all platforms, but builds a whole media stack (AEC, device UX, A/V sync,
native tile window) before the first call connects. Rejected for v1; it is
the recorded escape hatch if the webview leg disappoints, and the
data-plane wire format below is deliberately gateway-agnostic so C can
replace B per-platform without touching the inter-node protocol.

## Architecture

Five layers, one per concern:

```
webview UI (tiles, controls)      consensus (chat module)
   │ getUserMedia + RTCPeerConnection │ CallJoin/CallLeave/CallSweep → roster
   │ 127.0.0.1 SDP over node HTTP     ▼
   ▼                              AdmissionPolicy (roster ∩ channel members)
node media gateway (str0m)            │ permits (peer, service, flow)
   │ RTP payloads ⇄ media frames      ▼
   └────────► data-plane Service::Voice/Video datagrams ◄────────┘
                     over the WG overlay (dt-*, ULA)
```

### 1. Consensus control plane (chat module, version-gated)

New `ChatMsg` variants (rejected by `execute` until the upgrade module's
activation height; follows the forge `set_active_version` dual-path pattern):

- `CallJoin { channel_id }` — author (origin pubkey) enters the channel's call
  roster. First join creates the call; there is no separate `CallStart`.
  Rejected if the channel is members-only and the author isn't a member, or
  if the roster is at `MAX_CALL_PARTICIPANTS = 8`.
- `CallLeave { channel_id }` — removes the author; an empty roster ends the
  call (roster record deleted). Idempotent.
- `CallSweep { channel_id, user }` — any channel member may evict a roster
  entry (crash cleanup, same permissive-for-now trust posture as
  `SetMembership`). Liveness is not consensus-observable, so sweeping is a
  social/UI action, not automatic.

State: `call_roster: BTreeMap<channel_id, BTreeMap<pubkey, joined_at_height>>`
staged/committed like the rest of chat state and folded into chat's `root()`.
`ChatQuery::Call { channel_id }` returns the roster; `ChatEvent` gains
call-joined/left entries for hooks. Ops are low-rate (joins/leaves only —
never per-frame, never mute toggles).

### 2. Data-plane media wire

- **Service registry** (append-only): add `Video = 3`. `Voice = 2` and its
  8-byte header stay byte-identical — the voice engine's wire surface is
  untouched.
- **Flows** (derived, handshake-free; sender identity is the transport
  `PeerId`, so flows don't encode the sender):
  - `FlowId::derive("chat/call/{channel_id}/mic")` — `Service::Voice`,
    payload = one 20 ms Opus frame behind the existing 8-byte header.
  - `FlowId::derive("chat/call/{channel_id}/cam")` — `Service::Video`.
  - `FlowId::derive("chat/call/{channel_id}/ctl")` — `Service::Voice` (the
    always-on baseline of a call, so control works in the audio-only slice),
    control datagrams (below).
- **Video header (16 bytes, new, video-only)**:
  `ver:u8 · flags:u8 (bit0 keyframe) · frame:u32 · frag_index:u16 ·
  frag_count:u16 · ts_ms:u32 · reserved:u16`. An encoded frame larger than
  one datagram payload is fragmented; a frame missing any fragment is dropped
  whole (no retransmit — datagram class semantics). A dropped keyframe or
  reference gap triggers a rate-limited `KeyframeRequest`.
- **Control messages** on the `ctl` flow (tiny fixed-layout datagrams):
  - `KeyframeRequest` — receiver → sender; sender maps it to a PLI on its
    local browser leg. Rate-limited (≥ 1 s between requests per sender).
  - `Beacon { muted, camera_on }` — 1/s per participant; drives ephemeral UI
    (mute badge, "connecting…", stale-participant dimming). Never consensus.
  - `RateHint { max_kbps }` — receiver-side loss report; the sender node
    aggregates (min across receivers) and steers its browser encoder via REMB
    on the local leg. v1 ladder: 1200/800/500/300 kbps, step down on > 2%
    loss over 5 s, step up after 30 s clean.

### 3. Admission

`CallAdmission` implements `AdmissionPolicy` in the node layer over finalized
chat state: `(peer, service, flow)` is permitted iff `flow` derives from some
channel whose committed call roster contains `peer`'s pubkey (and, for
members-only channels, `peer` is a channel member). The node maintains the
channel→flow reverse map from the same finalized state. Leave/sweep revokes
admission at the next finalized block — a kicked peer's datagrams then drop at
demux, counted and attributed, per the plane's default-deny.

### 4. Node media gateway (in `noded`, off-consensus)

A per-call `CallSession` owned by the daemon (node-local seam on `NodeHandle`,
exactly like forge's smart-HTTP — no consensus surface):

- **Local leg**: `POST /v1/call/{channel_id}/session` — body: SDP offer from
  the webview; response: SDP answer. Re-POST with a new offer renegotiates —
  the app re-offers on camera toggle and on roster growth (it watches roster
  deltas and adds one `recvonly` transceiver per remote peer, so the browser
  side always initiates). One session per app client per call. str0m answers with
  host-candidate `127.0.0.1`, ICE-lite-style; DTLS/SRTP terminate here.
  **Codec pin**: Opus 48k mono + VP8 (universal across WKWebView/WebView2/
  Chromium; simple keyframe semantics; H.264 revisit later). 720p30 cap.
- **Egress**: depacketize local RTP → reassemble encoded frames → fragment
  into data-plane datagrams → fan out to each roster peer (mesh; uplink =
  bitrate × (N−1), bounded by the 8-cap and the rate ladder).
- **Ingress**: reassemble per-peer frames → packetize as RTP into the local
  leg (one recv-only transceiver per remote participant, `mid` = peer pubkey
  hex so the UI can label tiles) → browser decodes, renders, and mixes audio
  natively. The native jitter buffer/mixer in `voice/` is NOT in this path —
  it remains the sim/testing substrate and the future headless-participant
  path.
- **RTCP mapping**: local PLI ↔ cross-node `KeyframeRequest`; REMB ↔
  `RateHint`; NACK/retransmit disabled on the loopback leg.
- **Events to UI**: extend the existing `/v1/ws` broadcast with
  `WsFrame::Call` (roster deltas from finalized blocks are already covered by
  the block→refresh loop; this carries only ephemeral beacon/link state).

### 5. App surface

- `app/src/domain/calls-client.ts` — typed client: submit join/leave/sweep,
  query roster, POST SDP, subscribe `WsFrame::Call`.
- ChatView: huddle chip in the channel header (roster avatars + join button),
  a call panel with the tile grid (one `<video>` per remote `mid`, local
  preview, mute/camera/leave controls). Registry stays untouched — calls are
  a chat-view feature, not a new module tile.
- Store: call state joins the existing reducer/actions pattern
  (`state.activeCall`, optimistic join via `submitTracked`).

## Platform plan

| Platform | Local leg | Plumbing |
|---|---|---|
| macOS (primary) | In-webview `RTCPeerConnection` | `src-tauri/Info.plist` usage strings; camera/audio-input entitlements; adopt wry `with_permission_handler` when released |
| Windows | In-webview, same code | `PermissionRequested` auto-grant; document the persisted-Block reset |
| Linux desktop | **Companion call window**: `chromium --app=http://127.0.0.1:{port}/v1/call/{channel}/ui` — a minimal self-contained call page (`include_bytes!` into noded) speaking the same session route | Soft dependency on an installed Chromium/Chrome; absent → joining still shows the roster but no media, and the UI says "install a Chromium browser for calls on Linux". This is also the QA surface (headless chromium + `--use-fake-device-for-media-stream` on the dev box / fleet) |

The web build (vite in a real browser) gets the full in-page experience for
free — same client code, same routes.

CSP: media flows are not `connect-src`-governed and the SDP POST targets the
already-allowed `http://127.0.0.1:*`; no CSP change is expected. If a `blob:`
media URL sneaks in, add `media-src blob:` then, not preemptively.

## Interaction with adjacent designs

- **Private messaging (E2EE channels, spec'd 2026-07-06)**: media is
  point-to-point between member nodes over WG — no intermediate node ever
  carries it, so there is no SFU-style third party to hide plaintext from.
  Plaintext exists only in the participants' own node processes, the same
  trust boundary that design already grants the local node (node-side epoch
  crypto). SFrame-style media E2EE is therefore not needed for parity; noted
  as future hardening only.
- **No-downtime upgrade**: call ops activate at a governance-scheduled height
  via `effective_version`; `Service::Video` is off-consensus and merely
  append-only; old nodes drop unknown-service datagrams at demux by design.
- **Voice engine**: unchanged on the wire; gateway path supersedes its mixer
  for human clients; it remains the paused-clock sim substrate and the
  intended path for future agent participants.

## Failure modes

- **No tunnel (failed punch is terminal)**: a roster peer with no WG tunnel
  is simply unreachable for media; beacons never arrive; UI shows the tile as
  "unreachable" rather than pretending. No relay fallback exists by design.
- **Loss**: whole-frame drop + rate-limited keyframe request; audio gaps play
  as silence (Opus PLC absent — inherited, acceptable at v1).
- **App crash / sleep**: roster entry persists until `CallLeave`/`CallSweep`;
  beacon absence dims the tile within ~3 s, so the stale entry is honest.
- **Node restart mid-call**: `CallSession` is process-local; the app re-POSTs
  its offer on reconnect (same recovery path as the existing ws reconnect).
- **Backpressure**: datagram class drop-oldest isolates a slow peer; the rate
  ladder bounds aggregate uplink; joins beyond 8 are rejected in `execute`.

## Testing

- **Data-plane sim**: extend the voice engine's paused-clock tests to video —
  fragmentation/reassembly, whole-frame drop under injected loss,
  keyframe-request rate limiting, admission revocation on sweep.
- **Gateway integration**: str0m loopback tests (offer/answer, RTP in →
  datagrams out → RTP in) without a browser; then headless chromium with fake
  devices against a two-node localnet, asserting frames arrive cross-node.
- **Consensus**: standard module op tests — join caps, members-only gating,
  version gating pre/post activation height, roster root stability.
- **QA**: fleet/tauri-debug drive of the Linux companion window; manual macOS
  pass for TCC prompts + entitlements on a notarized build.

## Slices (implementation order — each lands independently)

0. **Data-plane overlay socket arm** (prerequisite, already a known gap):
   `DataPlaneTransport` over UDP on the `dt-*` ULA + `DataPlane` construction
   and admission wiring in `bin/node`/`bin/noded`.
1. **Consensus call ops + roster UI**: join/leave/sweep, version-gated;
   huddle chip shows who's "in the call" — no media yet, already useful.
2. **Audio concall**: gateway with Opus-only sessions over `Service::Voice`
   (no fragmentation needed — Opus frames fit one datagram), beacons, mute.
3. **Video**: `Service::Video`, 16-byte header, fragmentation, PLI/RateHint,
   tile grid.
4. **Platform plumbing**: macOS Info.plist/entitlements; Windows permission
   handling; Linux companion window + QA harness.
5. **Later**: screenshare, recording via blob-plane manifests, agent
   participants over the native engine, codec renegotiation.

## Decisions taken (flag to veto)

1. Calls live **inside the chat module** (new module id = consensus-breaking).
2. Media path is **node-gateway + data-plane**; no browser ICE/STUN/TURN
   anywhere; the coordinator is untouched.
3. **str0m** for the node-side WebRTC termination; **Opus + VP8 pinned**.
4. **Linux gets a Chromium companion window**, not in-webview media, and this
   doubles as the QA path; macOS/Windows are in-webview.
5. Roster is consensus; **mute/camera/liveness are ephemeral beacons**;
   crash cleanup is a member-initiated sweep op, not automatic expiry.
6. Slice 0 (data-plane socket arm) is in scope as the prerequisite.
