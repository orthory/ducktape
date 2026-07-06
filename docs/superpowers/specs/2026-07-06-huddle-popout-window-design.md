# Huddle pop-out window (Slack-style)

**Date:** 2026-07-06 · **Status:** approved ("lgtm. implement now")

## Problem

The huddle session card (`HuddleDock`) floats over every screen (`ConsoleShell`,
absolute bottom-left, z-25) so a hot mic never loses its mute/leave controls.
With the error-state message row it grew tall enough to overlap the rail —
and the user wants the Slack affordance anyway: pop the huddle into its own
small desktop window.

## Decisions (user-picked)

1. **Card + optional pop-out (Slack-exact).** The in-app card stays the
   default surface and the main window keeps owning the audio session
   (mic graph + voice ws). The popped window is a remote control.
2. **Pure event mirror.** The window renders only what the main window pushes
   over the Tauri event bus and sends commands back. No second node client.

## Surfaces & flow

- The card gains a **pop-out** button (Tauri only; web preview never shows it).
- Pop-out invokes Rust `huddle_pop_out` → creates/shows the `huddle` window
  (`index.html?view=huddle`, ~300×168, non-resizable, normal chrome) — same
  pattern as the tray popover.
- **While popped, the in-app card unmounts** (overlap relief). Closing the
  window (any way) brings the card back. Leaving the huddle closes the window.

## Event protocol (existing `ducktape://` convention, `core:default` covers it)

- `ducktape://huddle-state` (main → window): `{channelName, status, error,
  muted, participants: string[]}` — emitted on voice-slice/roster change and
  replayed on the window's `ready` handshake.
- `ducktape://huddle-cmd` (window → main): `{op: "set-muted", muted}` |
  `{op: "leave"}` | `{op: "retry"}` | `{op: "ready"}` — mapped onto existing
  store actions (`setHuddleMuted` / `leaveHuddle` / `joinHuddle`).
- `ducktape://huddle-closed` (Rust → main): fired on the huddle window's
  Destroyed event so the card re-mounts no matter how the window died.

## Components

- `views/chat/HuddleCard.tsx` (new): presentational card body (header dot +
  #channel + count, error row + Retry, mute/Leave controls, participant pile
  from display-name strings) — props + callbacks only, no store. Dock and
  window both render it so the two surfaces cannot drift.
- `views/chat/Huddle.tsx`: `HuddleDock` becomes a thin container (store →
  props); returns null while `voice.popped`.
- `views/huddle/HuddleWindow.tsx` (new): standalone `?view=huddle` view (no
  provider) — listens for state, sends commands, renders `HuddleCard`.
- `main.tsx`: route `?view=huddle` (mirrors `?view=tray`).
- `store`: `VoiceSlice` gains `popped: boolean`; actions gain
  `popOutHuddle`/`popInHuddle`; a provider effect emits state while popped and
  a listener maps window commands onto actions. Pure helpers
  (`buildHuddleWindowState`, cmd→action mapping) live in
  `store/huddle-window.ts` and are unit-tested.
- Rust: `src-tauri/src/huddle.rs` with `huddle_pop_out` / `huddle_pop_in`
  commands (tray.rs pattern); `capabilities/default.json` windows +=
  `"huddle"`.

## Edges

- Window shows "connecting…" until the handshake state lands.
- Main-window reload kills the session (existing behavior); boot closes any
  stale huddle window.
- Mic-denied error mirrors fully (message + Retry) into the window.

## Testing

- Unit: `buildHuddleWindowState` payload builder; command mapping.
- Live: tauri-debug drive — pop out, card unmounts, window renders, mute
  round-trips, leave closes the window.
