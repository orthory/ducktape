# ADR: Video Calls in Chat ("concall") — direction fixed, build not scheduled

Date: 2026-07-06 · Status: **Built** (2026-07-07, `feat/video-calls`) — see
`docs/superpowers/specs/2026-07-06-video-call-build.md` for what shipped vs
this text. Structural deviations: the webview↔node leg is the huddle build's
typed websocket + WebCodecs, not the §4 str0m WebRTC gateway (the inter-node
wire below shipped as written); admission is the huddle build's locally-live-
flows posture, not §3's consensus-roster policy (still the hardening seam);
and media rides the authenticated TCP mesh arm rather than waiting on the
slice-0 overlay socket, which remains a drop-in behind the same trait.

Original status: Accepted as direction, build not scheduled —
recorded so the architecture is not re-litigated when calls become current.
The gating prerequisite (the data-plane's real transport arm) is called out
below; nothing else about the design is time-sensitive.

## Context

Voice + video conference calls attached to chat channels ("huddle" model:
roster in the channel header, tile grid, mute/camera). What already holds and
the design leans on:

- **Data-plane** (`crates/system/data-plane/`): off-consensus byte transport
  designed for the WireGuard overlay (`dt-*`, fd::/48 ULA, per-peer /128).
  Datagram class (latency-first, drop-oldest, `MAX_DATAGRAM = 1372`) + paced
  stream class; append-only `Service { StateSync = 1, Voice = 2 }` registry;
  default-deny `AdmissionPolicy (peer, service, flow)` derived from finalized
  consensus state; identity = transport `PeerId` (WG cryptokey routing), no
  SSRC to spoof. **Sim transport only** — the real overlay-socket arm is
  unbuilt and no production binary constructs a `DataPlane`.
- **Voice engine** (`crates/apps/chat/src/voice/`): Opus/48k/mono/20 ms over
  `Service::Voice` behind an 8-byte header; jitter buffer + native mix;
  sim-proven, no capture/playback/AEC wired. Its docs already point at chat
  membership driving `AdmissionPolicy`.
- **Chat module**: channels, origin-bound authors, `SetMembership` +
  `PostPolicy::MembersOnly`. **Module-set invariant**: `global_root` hashes
  the module count and every `(id, root)` — a new module id forks at block
  one; new op variants in an existing module are also divergent for old
  binaries, so they ride the height-gated upgrade path
  (`Env::protocol_version`).
- **NAT layer**: coordinator is bespoke STUN-rendezvous only (not RFC 5389 —
  unusable by a browser ICE agent); failed punch is terminal; no relay
  (removed deliberately, PR #173). The shareable substrate is the WG tunnel.
- **Webview WebRTC ground truth (researched 2026-07-06)**: stock Linux
  WebKitGTK ships **no `RTCPeerConnection`** (`ENABLE_WEB_RTC` compiled out;
  Debian 2.53.4 verified; wry never enables the media settings either) — a
  custom WebKitGTK is not shippable and still a ~55%-WPT stack. macOS
  WKWebView works given `NSCamera/MicrophoneUsageDescription` +
  hardened-runtime entitlements; Windows WebView2 is Chromium and works.
  Precedent: Hopp (Tauri pair-programming app) went native-media for exactly
  the WebKitGTK reason.

## Options considered

- **A. Browser-P2P WebRTC mesh** (pairwise `RTCPeerConnection`s, SDP/ICE
  through consensus ops, STUN/TURN infra). Rejected: dead on Linux, needs
  the TURN relay the project just removed, re-solves NAT traversal the WG
  overlay already solved, and bypasses the data-plane and its admission
  model entirely.
- **B. Node media gateway + data-plane backbone — the chosen direction.**
  The webview holds exactly one `RTCPeerConnection`, to **its own local
  node** over `127.0.0.1` (host candidates only — no STUN/TURN/trickle/mDNS).
  The node terminates WebRTC (str0m: sans-IO, server-grade, fits the daemon's
  actor style) and repacketizes encoded frames onto data-plane datagrams to
  each roster peer. **SDP never crosses the network** — cross-node signaling
  is consensus roster ops + handshake-free `FlowId::derive`. The browser
  contributes capture, hardware codecs, AEC/AGC, rendering; the data-plane
  contributes transport, identity, admission.
- **C. Fully-native media engine** (GStreamer capture/encode + native render
  window; Hopp's end state). One pipeline everywhere and the best latency,
  but builds a whole media stack (AEC, device UX, A/V sync, native tiles)
  before the first call connects. The recorded escape hatch if a webview leg
  disappoints — the inter-node wire below is gateway-agnostic on purpose, so
  C can replace B per-platform without a wire change.

## Decision

1. **Calls live inside the chat module** (new module id = consensus-breaking).
   Ops, version-gated: `CallJoin { channel_id }` (first join creates the
   call; members-only gating; `MAX_CALL_PARTICIPANTS = 8`), `CallLeave`
   (empty roster ends the call), `CallSweep { channel_id, user }` (any
   channel member evicts a stale entry — liveness isn't consensus-observable,
   so cleanup is social, mirroring `SetMembership`'s current trust posture).
   Roster = `BTreeMap<channel_id, BTreeMap<pubkey, joined_at_height>>` in
   chat's `root()`. Consensus carries joins/leaves only — never media, never
   mute toggles.
2. **Media wire** (append-only commitments): `Service::Video = 3`;
   `Voice = 2` and its 8-byte header stay byte-identical. Flows per channel —
   `chat/call/{id}/mic` (Voice, one Opus frame/datagram),
   `chat/call/{id}/cam` (Video), `chat/call/{id}/ctl` (Voice, so control
   works in an audio-only build). Video header (16 B): `ver·flags(keyframe)·
   frame:u32·frag_index:u16·frag_count:u16·ts_ms:u32·reserved:u16`; frames
   fragment across datagrams, any missing fragment drops the whole frame.
   Control datagrams: `KeyframeRequest` (rate-limited ≥1 s),
   `Beacon { muted, camera_on }` (1/s, drives ephemeral UI),
   `RateHint { max_kbps }` (receiver loss report; sender takes min across
   receivers; ladder 1200/800/500/300 kbps).
3. **Admission**: node-layer `AdmissionPolicy` over finalized chat state —
   permitted iff the flow derives from a channel whose committed call roster
   contains the peer (∩ channel membership for members-only). Revocation
   lands at the next finalized block; unadmitted traffic drops at demux.
4. **Node media gateway in `noded`** (node-local seam on `NodeHandle`, like
   forge's smart-HTTP): `POST /v1/call/{channel_id}/session` = SDP offer in,
   answer out; the app re-offers on camera toggle and roster growth (one
   `recvonly` transceiver per remote peer, `mid` = peer pubkey hex). Codec
   pin **Opus + VP8**, 720p30 cap. RTCP maps across the boundary: local PLI ↔
   `KeyframeRequest`, REMB ↔ `RateHint`; NACK off on loopback. Ephemeral
   call events ride the existing `/v1/ws` as `WsFrame::Call`.
5. **Platforms**: macOS/Windows in-webview (Info.plist + entitlements /
   `PermissionRequested` handling). **Linux desktop = Chromium companion
   window** (`chromium --app=` against a minimal call page `include_bytes!`d
   into noded, same session route) — soft dependency, roster-only without
   it; this window is also the QA path (headless chromium, fake devices).
   The web build gets the full in-page experience with the same client code.
6. **Slice order when built**: (0) the data-plane overlay-socket arm — the
   prerequisite for any live media, independently useful to voice and
   statesync; (1) consensus call ops + roster UI, no media; (2) audio
   concall over `Service::Voice`; (3) video (fragmentation, PLI, tiles);
   (4) platform plumbing. Screenshare, recording (blob-plane manifests),
   agent participants (native voice-engine path), simulcast/SVC: explicitly
   later.

## Consequences

- **No new NAT machinery ever enters the design**: a peer without a WG
  tunnel (failed punch is terminal) is simply unreachable for media and the
  UI must say so. No relay, no TURN, coordinator untouched.
- **Trust boundary matches private messaging**
  (`docs/adr/2026-07-06-private-team-messaging.md`): media is point-to-point
  between member nodes; plaintext exists only inside participants' own node
  processes; no third-party hop ⇒ SFrame-style media E2EE is future
  hardening, not a parity requirement.
- Mesh topology bounds scale: uplink = bitrate × (N−1), capped at 8
  participants; larger calls need an SFU-shaped rethink (out of scope).
- Wire commitments are append-only from day one: `Service::Video = 3`, the
  16-byte video header, and the flow labels are the compatibility surface;
  old nodes drop unknown-service datagrams at demux by design.
- The webview leg inherits browser realities: Opus PLC absent in the native
  engine is irrelevant here (browser decodes), but macOS TCC prompts,
  notarized-build entitlements, and wry's unreleased `with_permission_handler`
  are real integration work (slice 4), not incidental polish.
- Until slice 0 exists, nothing here is buildable end-to-end — treat the
  data-plane transport arm as the trigger to reopen this ADR and write
  implementation plans.

## Full design detail

The complete spec this ADR condenses (evidence dossier with sources, layer
diagram, failure modes, test plan, code anchors) lived at
`docs/superpowers/specs/2026-07-06-video-call-module-design.md` and is
preserved in git history: added in 5ae8235, removed in the commit that adds
this ADR (branch `feat/video-call-design`).
