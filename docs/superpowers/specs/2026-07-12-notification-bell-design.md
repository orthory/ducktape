# Notification bell + dropdown — design

2026-07-12. Approved by the user (option: build as designed).

## Problem

The desktop notifier (PR #311, verified live 2026-07-12) presents transient OS
toasts and platform badges (macOS dock/tray, Linux window title from PR #433),
but the app itself has no notification surface: `notify-client.ts` exports
`onUnread` that nothing consumes, and a missed toast leaves no reviewable
trace inside the app on any platform.

## Solution

A bell icon in the `TitleBar` right cell (before the status dot), desktop-only
(`isTauri()`), identical on macOS/Linux/Windows, with an unread-count badge and
a dropdown listing recent notifications.

### Rust (`app/src-tauri/src/notify/`)

- `StoredNotification { category, title, body, channel_id, at }` — `at` is
  epoch millis stamped by the engine at present time. Serializable.
- The engine gains a capped ring (50, newest first) of presented notifications,
  held in an `Arc<Mutex<VecDeque<StoredNotification>>>` shared through
  `NotifyHandles` so the command below can read it without actor plumbing.
- `state.json` becomes `{ unread, recent }` (`serde(default)` — old files
  load). The list persists so the dropdown matches the badge after a restart.
- New command `notify_recent() -> { unread, items: Vec<StoredNotification> }`.
  The unread count rides along because the engine's boot-time badge event
  fires before the webview subscribes — after a restart with persisted unread
  the bell would otherwise show zero until the next live event.
- `AppSink::present` additionally emits each item as `ducktape://notify-item`
  so an open webview updates live.

### Webview

- `notify-client.ts` gains `recent()` and `onItem(cb)`.
- New `NotificationsBell` component (layout/): self-contained local state
  (items, unread) fed by `recent()` on mount plus `onItem`/`onUnread` events.
  No store changes. Hidden on web.
- Dropdown: anchored panel, newest first — category icon, title, clamped body,
  relative time. Click-outside/Esc closes. Empty state text.
- Item click navigates with what exists: `channelId` → chat channel, routed
  through `parseItemChannelId` when it names a hidden forge-item channel;
  no channel → category fallback (runs→agents, forge→forge,
  governance→members, chat/huddle→chat). Thread-level precision is a known
  ceiling (the deep-link target machinery stays deleted).
- New `bell` glyph in `Icon.tsx`.

### Behavior change (approved)

`markSeen` moves from the window-focus handler to dropdown-open. Focusing the
window no longer clears the badge; opening the bell does. All badge surfaces
(bell, macOS dock/tray, Linux window title) now persist until the bell is
opened. Focus-suppression of the actively viewed channel is unchanged.

## Tests

- Rust: ring cap/order, persistence round-trip of `{unread, recent}`,
  old-format `state.json` still loads.
- TS (vitest): bell renders the count, opening marks seen, item click
  navigates, forge-channel rerouting.

## Out of scope

Per-item read state, thread-level deep links from items, web-build bell,
notification history beyond 50, OS toast click actions.
