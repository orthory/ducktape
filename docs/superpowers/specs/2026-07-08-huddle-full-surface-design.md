# Huddle: Full Voice + Video Surface — Design

**Date:** 2026-07-08
**Status:** Approved (scope: "Full huddle surface", staged PRs)
**Target OSes:** macOS (WKWebView) and Ubuntu (WebKitGTK)
**Branching:** worktree off `origin/dev`; each PR based on `dev`.

## 1. Context & Goal

The voice + video huddle **backend transport and browser client are already built and
verified**: audio + camera video + call-control cross the real 2-node mesh byte-exact
(PRs #189, #261), and the *unmodified* `call-session.ts` client was proven end-to-end in
headless Chromium. What is **not** finished is the **interface and platform integration**:
capabilities that exist in the verified backend are unreachable from the real desktop UI,
and the video path is gated off on one target OS by a stale capability assumption.

This effort makes the huddle **feel real and fully integrated** on both macOS and Ubuntu —
polishing the interface into a complete voice + video call surface — without touching
consensus or protocol code.

## 2. Reframe: video is cross-platform (verification-grounded)

The code, its comments, and prior notes all assume *"WebKitGTK has no WebCodecs → Ubuntu
video degrades to audio + roster."* **This is obsolete.** Empirically verified on this box:

- **WebKitGTK 2.52.3** exposes `VideoEncoder`, `VideoDecoder`, `VideoFrame`,
  `requestVideoFrameCallback`, `getUserMedia`, and `getDisplayMedia` — **by default, no
  flag**. (Probed with `WebKit2.WebView` default settings.)
- The WebKitGTK video codec path is **GStreamer-backed**: the WebCodecs API is present, but
  actual VP8 encode/decode needs the `gstreamer1.0-plugins-good` (libvpx `vp8enc`/`vp8dec`)
  plugin installed at runtime. On this dev box those plugins are **absent**, so API presence
  alone does not imply working video.
- macOS WKWebView (Safari 16.4+) ships WebCodecs; the only shell-side blocker to macOS video
  is a missing `NSCameraUsageDescription`.

**Consequence:** video is a first-class feature on **both** target OSes. Two design
obligations follow:

1. Capability detection must probe *actual codec support*, not mere API presence
   (`VideoEncoder.isConfigSupported`), so a WebKitGTK box without VP8 plugins degrades
   honestly instead of showing a dead camera toggle.
2. The VP8 GStreamer plugin is a documented Ubuntu runtime dependency.

### Verification reality

- **Ubuntu**: buildable + verifiable end-to-end in this environment (after installing the
  VP8 GStreamer plugins on the box). This is the primary verification target.
- **macOS**: the `Info.plist` + entitlements changes are correct-by-spec, but this
  environment is headless Linux with **no Mac** — macOS camera/video must be verified by the
  user on real hardware. Every macOS-specific change is inert behind the capability gate
  until verified.

## 3. How the huddle works today (end-to-end)

Two planes: a **consensus roster** and an **off-consensus media** plane.

1. **UI entry** — `HuddleHeaderButton` / `HuddleRailBadge` (`app/src/console/views/chat/Huddle.tsx`,
   mounted from `ChatView.tsx`) call `actions.joinHuddle(channelId)`
   (`app/src/console/store/actions.ts:~1094`). Joins start **muted** by design.
2. **Consensus roster** — a `JoinHuddle` op (`crates/apps/chat/src/interface.rs`,
   `lib.rs`) appends `HuddleMember{user,node,joined_at}` to `Channel.huddle`
   (cap `MAX_HUDDLE_MEMBERS=32`). No media/timing/presence touches consensus. Read back only
   via the generic channel query (no dedicated huddle query/event).
3. **Media socket** — the webview client `app/src/domain/call-session.ts` opens one typed
   WebSocket `/v1/call/ws` and starts an Opus AudioWorklet graph + (if capable) a WebCodecs
   VP8 encoder. Send-set derives from `huddleRecipients(roster, selfNodeHex)`
   (`app/src/domain/voice-session.ts`), re-pushed on roster change (`actions.ts pushRecipients`).
   Binary framing (`0x01` audio, `0x02` captured video, `0x03` peer video) mirrored in
   `app/src/domain/call-frames.ts`.
4. **Node gateway** — `/v1/call/ws` in `bin/noded/src/lib.rs` (`call_ws`, `call_session`);
   returns 503 if no hub.
5. **Call hub** — `bin/node/src/voice.rs`, one session at a time (Slack "one huddle"),
   bridges the socket to the mesh over `CHANNEL_VOICE=7` (audio + control) and
   `CHANNEL_VIDEO=8` (camera fragments) (`bin/node/src/main.rs:199`). Fans out by roster
   node-key; per-peer reassembly; keyframe-request self-heal; 1 Hz presence beacons; 4-rung
   REMB rate ladder `[1200,800,500,300]`.
6. **Wire codecs** — `crates/apps/chat/src/video/{frame,assembly,control}.rs` and the Opus
   engine in `chat::voice`.
7. **Playout** — peer audio server-mixed into one stream; peer video decoded per-peer into
   `<canvas>` tiles (`Huddle.tsx TileGrid/PeerTile`). Beacons update
   `VoiceSlice.peers{muted,cameraOn,atMs}` (`app/src/console/store/state.ts:38`).

**Pop-out** — `actions.popOutHuddle()` → `huddle_pop_out` (`app/src-tauri/src/huddle.rs`)
creates a 300×168 companion window at `index.html?view=huddle`. It is a **pure event
mirror** (`HuddleWindow.tsx`): media never leaves the main webview; state crosses
`ducktape://huddle-state`, commands return over `ducktape://huddle-cmd`, and window-destroy
emits `ducktape://huddle-closed` so the in-app dock re-mounts. **It renders audio-only** — no
tiles, no camera.

## 4. Gap inventory

### Integration gaps (verified backend unreachable from the real UI)
- **`SweepHuddle` only reachable via a video tile's "stale·remove" chip**, which renders only
  when tiles show — so in an audio-only huddle a dead/ghost member can never be removed.
- **Per-peer mute state** rides beacons but is shown only on video tiles — invisible in the
  audio roster.
- **macOS camera blocked**: `Info.plist` has `NSMicrophoneUsageDescription` but no
  `NSCameraUsageDescription` → camera denies/crashes under TCC.
- **Ubuntu video gated off** by API-presence assumption though the runtime supports it.
- **Pop-out drops all video** (audio-only mirror).

### Polish gaps
- No **active-speaker** indication (self is client-doable via `AnalyserNode`; peer needs a
  backend energy beacon — deferred).
- No **screen-share** (though `getDisplayMedia` works on both OSes).
- No **device pickers** (mic/camera/speaker).
- Cramped ~200px dock — no real video **stage** for an actual call.
- `PeerTile` renders "muted" whenever `beacon?.muted !== false`, so **"no beacon yet" reads
  as muted**.
- Tiles beyond `MAX_VIDEO_PARTICIPANTS=8` are **silently dropped** (no "+N more").
- `PeerTile` keys the canvas on `beacon?.cameraOn` alone → a viewer that can't decode draws a
  **black `<canvas>`** instead of the initials avatar.
- Only `connecting/live/error` states — no distinct **reconnecting** or quality signal; the
  session ends on ws close with no retry.

### Named-but-deferred (out of scope for these PRs)
- **Peer active-speaker** — needs a backend per-peer energy beacon (audio is server-mixed;
  the client cannot attribute energy). Requires extending `control.rs` Beacon + `CallEvent`.
- **Server-side roster admission** — `AdmissionPolicy::permits` ignores the peer
  (`bin/node/src/voice.rs:196`): with a live session the node mixes *any* authenticated
  member's audio into your playout, roster or not. Privacy posture; ADR-deferred; consensus-
  adjacent.
- **Node-side liveness reaper** (auto-sweep dead members) and **roster change events**
  (replace block-poll re-query).

## 5. Staged delivery — four PRs

Each PR is based on `dev`, independently shippable, and verifiable on Ubuntu in-app. All
four are **client + shell only** — no consensus/lockstep risk. The one exception is a single
optional field on the off-consensus beacon control in PR 3 (screen-share), which does not
touch consensus.

- **PR 1 — Cross-platform video unblock + audio-huddle correctness.** The foundation.
- **PR 2 — Huddle stage + pop-out video.** The real video-call surface.
- **PR 3 — Screen-share.** `getDisplayMedia`, mode-swap on the single video lane.
- **PR 4 — Device pickers + connection quality/reconnecting states.**

## 6. PR 1 — Cross-platform video unblock + audio-huddle correctness (detailed)

Goal: video correctly enabled/detected on both OSes, and the verified backend ops reachable
in **every** huddle (including audio-only). Ships a correct, integrated huddle on both OSes.

### 6.1 Capability probe (real codec support, not API presence)
- Replace synchronous `supportsVideoCalls()` (`call-session.ts:90`, API-presence only) with
  an **async capability probe** that runs once and caches:
  `VideoEncoder.isConfigSupported({codec:'vp8', width:1280, height:720, bitrate:800_000, framerate:30})`
  → `{supported}`; also confirm `VideoDecoder.isConfigSupported({codec:'vp8'})`.
  Keep the existing API-presence checks (`VideoEncoder`/`VideoDecoder`/`rVFC`/`getUserMedia`)
  as the fast pre-gate; only call `isConfigSupported` when those pass.
- Expose it through the store as an async-resolved boolean the UI reads
  (`actions.videoSupported()` today is sync — introduce a resolved `voice.videoCapable`
  cached flag on the state, computed at app start and after daemon-ready).
- **Degrade honestly**: when the API is present but codec unsupported (WebKitGTK without VP8
  plugins), hide the camera toggle and show the existing "Video needs …" hint reworded to
  "Camera needs the VP8 video codec (install gstreamer1.0-plugins-good)".
- Files: `app/src/domain/call-session.ts`, `app/src/console/store/{state.ts,actions.ts}`,
  `app/src/console/views/chat/Huddle.tsx`.
- Tests: unit-test the probe's decision table (API-absent, API-present+codec-unsupported,
  fully-supported) with a mocked `VideoEncoder.isConfigSupported`.

### 6.2 macOS camera unblock
- Add `NSCameraUsageDescription` to `app/src-tauri/Info.plist`.
- Add a macOS entitlements plist (`com.apple.security.device.camera`,
  `com.apple.security.device.audio-input`) and reference it from
  `tauri.conf.json bundle.macOS.entitlements` for hardened-runtime signed builds.
- **Inert until verified**: guarded by the capability gate; no behavior change on Ubuntu.
- Verification: user runs on a real Mac (documented in the PR).

### 6.3 Reachable roster management in every huddle
- Lift the **roster list**, the **beacon-staleness tick**, and the **`SweepHuddle` control**
  out of the `showTiles` gate (`Huddle.tsx:384–394`) into the shared `HuddleCard` so they
  render in audio-only huddles. Design: `HuddleCard` gains an optional roster-rows section
  (name + per-member mute glyph + a "remove" affordance shown when a member's beacon is
  stale). The dock and pop-out both get it.
- Render **per-peer muted glyph** on the roster rows (beacons already carry it).
- Files: `app/src/console/views/chat/HuddleCard.tsx`,
  `app/src/console/views/chat/Huddle.tsx`, `app/src/console/store/huddle-window.ts`
  (window state must carry per-member `{name, muted, node, stale}` instead of just names).
- Tests: `buildHuddleWindowState` projection includes mute/stale; `applyHuddleWindowCmd`
  gains a `sweep` command; staleness decision reused (`isBeaconStale`).

### 6.4 Correctness fixes
- **Blank-tile guard**: gate the peer `<canvas>` render branch on `voice.videoCapable`
  (`Huddle.tsx:294`) so a viewer that cannot decode shows the initials avatar, not a black
  canvas.
- **"+N more"**: when `roster` exceeds `MAX_VIDEO_PARTICIPANTS`, show a "+N more" chip on the
  grid and in the roster (`Huddle.tsx TileGrid`, `HuddleCard`).
- **Distinguish peer states**: split `muted` / `unknown-beacon` / `active` so "no beacon yet"
  no longer reads as muted (`Huddle.tsx PeerTile` + roster rows).

### 6.5 Self active-speaker + muted-while-talking banner (client-only, both OSes)
- Add a local `AnalyserNode` on the mic capture graph in `call-session.ts`; emit a throttled
  `selfLevel` (or boolean `speaking`) via a new `CallEvent` kind. No wire change.
- Self-tile / self roster row shows a **speaking ring**; when energy is detected **while
  muted**, show a prominent "You're muted" banner in the card.
- Files: `call-session.ts`, `voice-session.ts` (pure helper for the threshold),
  `Huddle.tsx`, `HuddleCard.tsx`, `state.ts` (`voice.speaking`, `voice.selfLevel`).
- Tests: pure VU/threshold helper unit-tested; banner logic (muted && speaking) unit-tested.

### 6.6 Verification (PR 1)
- App unit tests (`bun test` / vitest) green, including new probe/projection/threshold tests.
- `cargo clippy -p <touched crates> --tests --no-deps` green for any Rust touched
  (huddle.rs / tauri shell if changed).
- Ubuntu end-to-end in the real app (tauri-debug / qa fleet): install VP8 GStreamer plugins,
  join a huddle, confirm camera toggle appears, self-preview renders, capability probe passes;
  confirm `SweepHuddle` + mute glyphs appear in an **audio-only** huddle; confirm blank-tile
  guard and "+N more".
- macOS: build correctness only; hand verification to the user with a checklist.

## 7. PR 2 — Huddle stage + pop-out video (outline)

- **Stage view**: a real expanded theater/gallery layout (responsive columns, spotlight +
  gallery toggle) replacing the cramped dock grid for an actual call. Spotlight driven by the
  self active-speaker signal (from PR 1) + camera/roster order. Reachable from the dock
  ("expand") and as the pop-out window's content.
- **Pop-out video**: fork a compact tile strip + camera toggle into the pop-out; make the
  window resizable and capability-aware; the main webview keeps owning the session (mirror
  frames, or move the render — decide in the plan). Add `always_on_top(true)` +
  `skip_taskbar(true)` in `huddle.rs`.
- Files: `HuddleWindow.tsx`, `huddle.rs`, `huddle-window.ts`, `Huddle.tsx`, new
  `views/huddle/HuddleStage.tsx`.

## 8. PR 3 — Screen-share (outline)

- `getDisplayMedia` capture as a **mode swap** on the existing single video lane: turning on
  screen-share turns off camera and encodes the screen track through the same VP8 WebCodecs
  path — **no wire/consensus change** for the media itself.
- Peers must render a shared screen "contain" (not "cover") at its real aspect. This needs a
  one-field `sharing` flag on the **off-consensus** beacon control: extend `control.rs`
  Beacon + the client beacon + `CallEvent`/`VoiceSlice.peers`. Off-consensus control plane;
  no lockstep.
- Simultaneous camera + screen (two video streams) is a **future wire enhancement** (a
  stream-id in the frame header) — explicitly out of scope here.
- Files: `call-session.ts`, `Huddle.tsx`/stage, `crates/apps/chat/src/video/control.rs`,
  `bin/node/src/voice.rs` (beacon passthrough), `bin/noded` framing if needed.

## 9. PR 4 — Device pickers + connection quality (outline)

- `enumerateDevices` → mic/camera/speaker selection (a small gear); `HTMLMediaElement.setSinkId`
  for speaker routing. Persist last-used device.
- Distinct **reconnecting** state: ws retry with backoff instead of terminal `closed`;
  reconnecting banner in the card.
- Per-peer **quality hint** from the rate ladder / dropped frames.
- Files: `call-session.ts`, `Huddle.tsx`, `HuddleCard.tsx`, `state.ts`.

## 10. Platform matrix

| Capability | macOS (WKWebView) | Ubuntu (WebKitGTK 2.52) |
|---|---|---|
| Audio | ✅ (`NSMicrophoneUsageDescription` set; once denied, macOS never re-prompts → route to System Settings) | ✅ shipped native path |
| Video | ⚠️ Architecturally reachable; blocked only by missing `NSCameraUsageDescription` (PR 1 fixes); **verify on real Mac** | ✅ WebCodecs present by default; needs VP8 GStreamer plugin at runtime; PR 1 probes real codec support |
| Screen-share | `getDisplayMedia` present (PR 3) | `getDisplayMedia` present (PR 3) |
| Pop-out video | PR 2 | PR 2 |
| Device pickers | PR 4 | PR 4 |

## 11. Risks & mitigations

- **WebKitGTK video works in a standalone probe but not the real Tauri/wry webview** →
  verify in the real app early in PR 1 (tauri-debug), before building UI on top.
- **VP8 GStreamer plugin missing on a user's Ubuntu** → real capability probe degrades
  honestly + documented dependency; camera hidden with a clear reason.
- **macOS unverifiable here** → keep macOS changes inert behind the gate; hand the user a
  verification checklist; do not claim macOS video works.
- **Pop-out video ownership** (mirror vs move the media session) → resolve in the PR 2 plan
  with a spike; keep the session single-owner in the main webview to avoid double-capture.
- **Scope creep across PRs** → each PR self-contained, reviewed from a clean context against
  `dev` before merge (per CLAUDE.md).

## 12. Out of scope (this epic)

Peer active-speaker beacon, server-side roster admission, node-side liveness reaper, roster
change events, simultaneous camera + screen-share. Each is noted as a follow-up.
