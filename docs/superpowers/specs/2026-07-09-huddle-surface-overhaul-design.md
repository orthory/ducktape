# Huddle Surface Overhaul + PR 2b/3/4 — Design

Date: 2026-07-09
Status: approved-pending-review
Scope: app UI + a small amount of node/shell (Rust) for the pop-out window and the
screen-share beacon field. No consensus / lockstep changes.

## Goal

Finish the huddle epic (video-in-window, screen-share, device pickers) **and**
overhaul the huddle layout so the three surfaces stop diverging. Primary target
OSes: macOS + Ubuntu. Backend transport + browser client are already verified
(see memory `call-transport-verified-callbed`, `video-call-module-design`); this is
interface + platform-enablement work, not protocol work.

## Problem (why the current design "sucks")

1. **Scattered buttons.** The dock splits controls across three places: pop-out is
   a tiny icon in the card header; mute + Leave sit in one row; expand + camera in a
   *separate* row below. Mic and camera — the two most-related controls — are in
   different rows, and destructive Leave sits directly beside Mute.
2. **Three divergent surfaces.** The dock (`TileGrid`, 2-col 16:9), the full stage
   (`StageTile` gallery/spotlight), and the popped window (audio-only card, *no
   video*) each render the same session differently — three implementations that
   drift, and a mental-model shift every time you expand or pop out.
3. **The floating window is a dead-end.** 300×168, audio-only, a third layout. If
   you pop out of a video huddle you lose all video.

## Decisions

Two load-bearing forks were put to the user; both are locked:

- **Dock form factor → "Compact card + one bar".** Keep the vertical dock card, but
  a small tile strip on top and *one* control bar at the bottom. Video stays
  glanceable in the dock; lowest-risk evolution.
- **Floating window → "Real video via session handoff".** The popped window hosts
  the full stage with live video and the same control bar; the media session moves
  between webviews on pop in/out (~sub-second audio reconnect). Always-on-top float.

Tailored calls (not vetoed individually, recorded here):

- One `HuddleControls` bar, fixed order `[Mic] [Camera] [Screen] [⋯ Devices]  ⟨grow⟩
  [Expand/Collapse] [Pop out/in] [Leave]`. Leave is danger-styled, isolated by a
  gap at the far right. Screen + Devices slots are present from PR-A but inert until
  PR-C/PR-D fill them.
- Screen-share is a **mode-swap** on the single VP8 video lane (camera XOR screen);
  simultaneous camera+screen is out of scope.
- Device selections persist in `localStorage` and are re-applied on rejoin.
- "Reconnecting" is a distinct call state from hard "error".

## Architecture

Collapse the three surfaces into one composition rendered at three sizes.

### Components (new / reshaped)

- **`views/huddle/HuddleControls.tsx` (NEW)** — the single control bar. Purely
  presentational: props are capability flags + state (`muted`, `cameraOn`,
  `sharing`, `live`, `reconnecting`, `home: "dock" | "stage" | "window"`) and
  callbacks (`onToggleMute`, `onToggleCamera`, `onToggleScreen`, `onOpenDevices`,
  `onExpand`/`onCollapse`, `onPopOut`/`onPopIn`, `onLeave`). A `size:
  "compact" | "comfortable"` prop scales paddings/glyphs. The Expand vs Collapse and
  Pop-out vs Pop-in buttons are chosen by `home`. This is the single place button
  order/spacing is defined, so it can never drift between surfaces again.
- **`views/huddle/CallTiles.tsx` (NEW)** — one tile renderer with `layout:
  "strip" | "gallery" | "spotlight"`. Absorbs the dock's `TileGrid` and the stage's
  gallery/spotlight branches. Reuses the existing pure `huddle-stage-layout.ts`
  (`galleryColumns`, `spotlightKey`). Exports the shared `StageTile` (self preview
  when `canEncode`+cameraOn, peer canvas when `canDecode`+beacon.cameraOn, else
  avatar; `sharing` beacons render `object-fit: contain` + a "screen" label in PR-C).
- **`views/huddle/CallSurface.tsx` (NEW)** — the shared composition: status header
  (dot + `#channel` + count) → error/muted banners → `CallTiles` (or the audio
  roster when no video) → `HuddleControls`. Takes the fully-resolved
  `HuddleParticipant[]` + capability + the callback bundle. Binds the media session
  through the callbacks passed by its container; it does not itself own the session.
- **`views/chat/Huddle.tsx` (reshaped)** — `HuddleDockCard` renders `CallSurface`
  with `size="compact"`, `layout="strip"`, `home="dock"`. The bespoke "dock controls
  row" (expand + camera) is deleted (folded into `HuddleControls`).
- **`views/huddle/HuddleStage.tsx` (reshaped)** — renders `CallSurface` full-window
  with `size="comfortable"`, gallery/spotlight toggle, `home="stage"`. Its bespoke
  control bar is replaced by `HuddleControls`.
- **`views/huddle/HuddleWindow.tsx` (reshaped in PR-B)** — renders `CallSurface`
  with `home="window"`; owns a real media session (see PR-B).
- **`views/chat/HuddleCard.tsx` (reshaped)** — reduced to the audio path: header +
  banners + roster rows + `HuddleControls`. The header pop-out icon is removed (pop
  moves into the bar). Kept because the audio-only roster (names + per-member mute +
  stale-sweep) is still the right compact body when there is no video.

### Single-owner media invariant

Exactly one mounted `CallSurface` may bind the session at a time. Today this is
enforced by early-returns (dock XOR stage; popped hides the dock). PR-B extends the
invariant across the process boundary: main window owns media when docked/expanded;
the popped window owns it when popped; the handoff transfers ownership with the WS
closed on one side before it opens on the other.

## PR-A — Layout overhaul (app-only, foundation)

Build `HuddleControls`, `CallTiles`, `CallSurface`; rewire dock + stage + audio
card onto them; delete the duplicated renderers. No behavior change beyond layout:
same actions, same capability gates, same roster. The popped window is *not* touched
here — it keeps its current audio-only mirror until PR-B (so PR-A ships independently
and safely). Net effect the user sees: coherent one-bar dock; identical bar in the
stage; Leave isolated.

Deliverables: the three new components + their unit/RTL tests; dock/stage/audio-card
rewired; old `TileGrid` (in `Huddle.tsx`) and the stage's inline tile/control code
removed. Gate: `bun run typecheck`, `bun run test`, `bun run lint` green.

## PR-B — Real video in the floating window (session handoff)

The popped window becomes a real media client instead of a pure mirror.

### Ownership model

- **Consensus roster** stays owned by the **main** window (it holds the node
  subscription). Roster/names continue to flow main→window over `huddle-state`.
- **Media** (WS `/v1/call/ws`, mic capture, camera/encode, peer decode, beacons,
  recipient fan-out) is owned by whichever window is active.

### Protocol extension (`store/huddle-window.ts`)

`HuddleWindowState` gains the fields the window needs to *start its own* session:
`nodeUrl`, `channelId`, `videoCapability`, and the initial `muted`/`cameraOn`.
`HuddleWindowCmd` gains `{op:"toggle-camera"}` / `{op:"toggle-screen"}` /
`{op:"devices", …}` so the window's controls drive its own session directly (they
no longer round-trip to main). A new `{op:"media-released"}` / `{op:"media-taken"}`
pair coordinates the handoff so only one WS is open per node at a time.

### Handoff sequence

- **Pop-out:** main `stopVoice()` (media only — no `submitLeaveHuddle`, `channelId`
  stays) → `huddle_pop_out` creates the window → window `ready` → main replays
  state incl. `nodeUrl`/`channelId`/capability → window `createCallSession` +
  `.start(callSocketUrl(nodeUrl, channelId))`, re-applies persisted mute/camera.
- **Pop-in / window closed:** window `stopVoice()` → `huddle-closed` → main
  re-`createCallSession` + `.start()` and resumes ownership.
- The audio gap is bounded to the WS reconnect. Camera state is re-applied from the
  mirrored `cameraOn`; mic starts muted-or-as-last per the persisted value.

### Window chrome

The window grows to a video-capable size (e.g. 360×260, resizable within limits),
keeps `always_on_top` + `skip_taskbar`, and hosts `CallSurface home="window"` with a
compact gallery. `huddle.rs` updates the size + resizable flags.

### Risks / verification limits

- **getUserMedia in a child webview.** Mic/camera permission is app-level
  (entitlements on macOS; portal on WebKitGTK), but a second webview acquiring the
  devices is unverified on this headless box. If the child cannot acquire, fall back
  to keeping media in main and mirroring (the current behavior) — detect a start
  failure in the window and emit `media-failed` so main re-takes ownership. This
  fallback must exist so pop-out can never strand the session.
- **1-session-per-node hub.** The ordering (close-before-open) is mandatory; add a
  short settle before the window opens its WS. Verify the hub tolerates the rapid
  close/reopen (it should — it is a rejoin).
- Roster membership must be observably unchanged across a pop-out/in cycle (no
  join/leave op emitted) — asserted in a test around the action wiring.

## PR-C — Screen-share

- `call-session.ts`: `setScreenShare(on)` acquires `getDisplayMedia({video:true})`
  and swaps the VP8 lane's source (camera XOR screen). A screen-tuned encoder config
  (higher resolution, lower framerate) if `isConfigSupported` accepts it, else reuse
  the camera config.
- Beacon control gains an optional `sharing: boolean` (off-consensus, additive — no
  lockstep). Peers render a `sharing` tile with `object-fit: contain` + a "screen"
  chip. Check `voice.rs` beacon relay passes unknown/optional fields through
  untouched (it relays opaque control JSON — confirm).
- `HuddleControls`: the Screen button becomes live. While sharing, the Camera button
  is disabled (mode-swap). Capability: needs `canEncode` **and**
  `getDisplayMedia` present — probe separately; hide the button when absent.
- Verification limit: `getDisplayMedia` on WebKitGTK depends on a working portal;
  gate + degrade rather than assume.

## PR-D — Device pickers + reconnect/quality

- **Pickers.** `enumerateDevices()` → a Devices menu (the `⋯` slot) with mic /
  camera / speaker lists. Mic/camera apply via a `deviceId` constraint on the next
  `getUserMedia`; speaker via `HTMLMediaElement.setSinkId`. Persist chosen ids in
  `localStorage`, re-apply on rejoin. Hide the speaker section when `setSinkId` is
  absent (WebKitGTK may lack it).
- **Reconnecting.** `call-session.ts` distinguishes a transient WS drop (auto-retry,
  status `reconnecting`) from a terminal failure (`error`). The surface shows a
  reconnecting banner (amber) instead of the red error card; Leave still works.
- **Quality (light).** A small indicator derived from beacon freshness / decode
  cadence — green/amber/grey dot per tile. No new backend signal; purely local
  heuristics. Keep minimal; do not over-build (see memory
  `telemetry-phase2-not-built`).

## Testing strategy

- Pure units: `HuddleControls` button-order/disabled-state matrix; `CallTiles`
  layout selection + tile source (self/peer/avatar/sharing); reuse
  `huddle-stage-layout.test.ts`.
- RTL: dock renders one bar with the fixed order; Leave isolated; expand → stage
  uses the same bar; audio-only huddle still exposes mute + sweep.
- PR-B: action-level test that a pop-out/in cycle emits no join/leave consensus op
  and preserves `channelId`; protocol builder/mapper round-trip for the new
  fields/ops; window-side start/stop is unit-tested behind a stubbed call session.
- PR-C: `setScreenShare` swaps source + sets `sharing` beacon; tile renders
  `contain` when a peer beacon says sharing.
- PR-D: device-id persistence round-trip; reconnecting status → banner not error
  card; setSinkId-absent hides the speaker section.

## Verification limits (whole epic)

This box has no VP8 **encoder** and no Mac, so real camera-send video, screen-share
capture, and macOS behavior can't be exercised here — all such paths stay
capability-gated and are marked for real-hardware QA. Decoder-side and all
non-media logic are testable headless (see memory `webkitgtk-webcodecs-reality`).

## Staging & order

PR-A (foundation, app-only) → PR-B (window handoff, app+shell) → PR-C (screen-share,
app+node beacon field) → PR-D (device pickers + reconnect/quality, app). Each is its
own worktree off `dev`, its own PR against `dev`, gates green, clean-context review
before merge — per repo `CLAUDE.md`.
