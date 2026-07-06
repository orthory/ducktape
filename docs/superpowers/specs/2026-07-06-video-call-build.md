# Video calls in huddles — what shipped vs the ADR

Built 2026-07-07 on `feat/video-calls`, as the follow-up to the chat-huddle
build (PR #178). This records where the implementation tracks
`docs/adr/2026-07-06-video-call-module.md` and where it deliberately deviates,
so the ADR stays honest without rewriting its history.

## What shipped

- **`Service::Video = 3`** in the data-plane registry (append-only,
  wire-stable).
- **`chat::video`** (`crates/apps/chat/src/video/`): the ADR's 16-byte video
  header — `ver·flags(keyframe)·frame_no:u32·frag_index:u16·frag_count:u16·
  ts_ms:u32·reserved:u16`, big-endian — with frame fragmentation across
  data-plane datagrams (`MAX_FRAGMENT_PAYLOAD = 1344`, `MAX_FRAGS = 96`,
  any missing fragment drops the whole frame), per-sender reassembly with
  monotonic wrapping `frame_no`, and the control codec:
  `KeyframeRequest` / `Beacon { muted, camera_on }` /
  `RateHint { max_kbps }`, ladder 1200/800/500/300 kbps.
- **`sweep_huddle` consensus op** — the ADR's `CallSweep`, named for the
  shipped `join_huddle`/`leave_huddle` family: any author the channel's post
  policy admits evicts a stale roster entry; absent target is a
  deterministic no-op. Consensus still carries joins/leaves/sweeps only —
  never media, never mute state.
- **Node hub video path** (`bin/node/src/voice.rs`): three datagram flows per
  session — mic `(Voice, voice-channel:{id})`, cam `(Video,
  video-channel:{id})`, ctl `(Voice, callctl-channel:{id})` — control rides
  `Service::Voice` so it works in an audio-only build. Fragmentation on
  capture, per-peer reassembly on receipt, PLI-shaped keyframe requests
  (≥1 s rate limit both directions), 1 Hz presence beacons, REMB-shaped
  rate control (receiver: 5 s loss windows, >10% steps down, 3 clean windows
  step up; sender: min hint across current recipients).
- **`CHANNEL_VIDEO = 8`** — video fragments ride their own mesh lane with
  their own per-peer quota (512/s ≈ 5.6 Mbps) so keyframe bursts never queue
  ahead of voice; black-holed off the validator path in observer/parked
  modes like voice.
- **`GET /v1/call/ws`** (replaces `/v1/voice/ws`): one socket for tagged
  binary media (`0x01` audio PCM, `0x02` captured video, `0x03` peer video;
  little-endian on this leg) and camelCase JSON control
  (`recipients`/`beacon`/`keyframeRequest` in; `keyframeRequest`/
  `peerBeacon`/`rateHint` out). Refusals still answer with a reason.
- **App**: `call-frames.ts` (tested codec of the socket framing),
  `call-session.ts` (the audio graph from #178 unchanged + a WebCodecs
  VP8 pipeline — encoder at 720p30 `realtime`, per-peer decoders gated on a
  sync point, camera failure degrades to audio-only), store slice
  (`cameraOn`, per-peer beacon state), and the huddle dock's camera toggle,
  2-column tile grid, self preview, and stale-peer sweep chip
  (no beacon for 10 s → "stale · remove" → `sweep_huddle`).

## Deviations from the ADR text

- **Gateway: WS bridge, not str0m WebRTC.** ADR §4 sketched a node-side
  WebRTC termination (`POST /v1/call/{id}/session`, SDP answer, RTCP
  mapping). The huddle build (#178) had already established a simpler
  webview↔node seam — the typed websocket — and WebCodecs covers capture/
  encode/decode without an `RTCPeerConnection`, so the SDP/str0m layer became
  pure overhead: same browser capabilities, same inter-node wire, one less
  protocol boundary. The ADR's inter-node wire is unchanged (it was declared
  gateway-agnostic on purpose); only the localhost leg differs. RTCP maps as
  PLI ↔ `keyframeRequest`, REMB ↔ `rateHint` over the socket instead of RTCP
  packets.
- **Flow labels** follow #178's shipped `voice-channel:{id}` convention
  (`video-channel:`, `callctl-channel:`) rather than the ADR's illustrative
  `chat/call/{id}/…` strings. The label is only a `FlowId::derive` domain
  string; both ends derive it from the channel id either way.
- **Ops are the huddle family** — `join_huddle`/`leave_huddle` (shipped in
  #178, roster on the `Channel` record) plus the new `sweep_huddle`, not the
  ADR's `CallJoin`/`CallLeave`/`CallSweep` with a separate roster map.
- **Caps**: the huddle roster cap stays 32 (#178); video is gated at the
  ADR's `MAX_VIDEO_PARTICIPANTS = 8` in the app (toggle disabled beyond 8).
  The mesh cap the ADR worried about (uplink = bitrate × (N−1)) binds the
  same either way.
- **Ephemeral call events ride the call socket**, not `/v1/ws`
  `WsFrame::Call` — the call socket already exists per session and dies with
  it, which is exactly the lifetime beacons need.
- **Transport**: media rides the authenticated TCP mesh via the hub's
  `ChannelTransport` (the #178 arm), not yet UDP-on-WireGuard; the plane
  seam (`DataPlaneTransport`) is unchanged, so the overlay-socket arm swaps
  in without touching any of this build.
- **Admission** matches #178's posture: locally-live flows (the operator's
  own sessions), not the ADR's consensus-roster-derived policy. The mesh
  already authenticates every peer as a workspace member; flow-level roster
  admission remains the hardening seam noted in the huddle spec. Concretely,
  the cost is that any workspace member — not just the channel's huddle roster —
  can inject media into a live session's flows, and the receiving app will spin
  up a decoder pipe for that un-rostered sender (bounded to workspace members;
  the flow-level roster-admission seam is what closes it).

## Consensus-adjacent note

`CHANNEL_VIDEO = 8` collided with the epoch-engine channel bank, whose base
was 8; the bank now starts at 9 (`engine_channels`). Mesh channel layout is a
lockstep contract — old and new binaries must not mix — which matches the
posture this branch already has (new module op = divergent for old binaries).

## Deferred (unchanged from the ADR)

- Linux desktop Chromium companion window (`chromium --app=` against a
  minimal call page): the app degrades to roster+audio when WebCodecs is
  missing (`supportsVideoCalls()`), which is the WebKitGTK case today. The
  web build in a Chromium browser gets the full experience now.
- macOS/Windows camera entitlements plumbing (ADR slice 4).
- Screenshare, recording, agent participants, simulcast/SVC, SFrame E2EE.
