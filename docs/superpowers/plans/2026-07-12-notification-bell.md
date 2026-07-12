# Notification Bell + Dropdown Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An in-app notification bell in the TitleBar (all desktop platforms) with an unread badge and a dropdown of recent notifications; badge-clearing moves from window-focus to dropdown-open.

**Architecture:** The Rust notify engine gains a persisted 50-item ring of presented notifications shared via `Arc<Mutex<VecDeque>>` between the stream task and a new `notify_recent` command; `AppSink` emits each item live as `ducktape://notify-item`. The webview gains a self-contained `NotificationsBell` component (no store changes) fed by `notify_recent` + the item/unread events.

**Tech Stack:** Rust (tauri 2.11 on `tauri-runtime-cef`), React + vitest.

**Spec:** `docs/superpowers/specs/2026-07-12-notification-bell-design.md`

## Global Constraints

- Worktree: `.worktree/notification-bell`, branch `notification-bell`, PR base `dev`.
- Rust gates: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps` and `ops/build-with.sh cargo test -p ducktape-desktop notify::`.
- TS gate: `cd app && bun run test` (vitest).
- Ring cap is 50, newest first. `at` = epoch milliseconds.
- Screen ids used by navigation: `chat`, `agent`, `forge`, `governance`.
- Do not touch focus-suppression of the actively viewed channel (engine `should_present`) — only seen-marking moves.

---

### Task 1: StoredNotification + persisted ring state (Rust)

**Files:**
- Modify: `app/src-tauri/src/notify/matchers.rs:10` (Category derives)
- Modify: `app/src-tauri/src/notify/state.rs` (NotifyState + StoredNotification)

**Interfaces:**
- Produces: `state::StoredNotification { category: Category, title: String, body: String, channel_id: Option<String>, at: u64 }` (Serialize/Deserialize camelCase, Clone, Debug, PartialEq); `NotifyState { unread: u32, recent: Vec<StoredNotification> }`; Category serializes lowercase (`"mention"`, …).

- [ ] **Step 1: Add serde derives to Category** in `matchers.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
```

- [ ] **Step 2: Add StoredNotification + recent to state.rs** (below `NotifyState`; extend the struct and its doc — `state.json` now carries the dropdown's list so it matches the persisted badge after a restart):

```rust
/// One presented notification kept for the in-app bell dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredNotification {
    pub category: super::matchers::Category,
    pub title: String,
    pub body: String,
    pub channel_id: Option<String>,
    /// Epoch milliseconds at present time.
    pub at: u64,
}
```

and in `NotifyState`:

```rust
pub struct NotifyState {
    pub unread: u32,
    /// Recent presented notifications, newest first, capped by the engine.
    pub recent: Vec<StoredNotification>,
}
```

(`#[serde(default)]` is already on the struct — an old `{"unread":n}` file loads with an empty list.)

- [ ] **Step 3: Add a round-trip + old-format test** at the bottom of `state.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::notify::matchers::Category;

    #[test]
    fn state_round_trips_and_old_format_still_loads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("state.json");

        let state = NotifyState {
            unread: 2,
            recent: vec![StoredNotification {
                category: Category::Mention,
                title: "t".into(),
                body: "b".into(),
                channel_id: Some("general".into()),
                at: 1,
            }],
        };
        save(&path, &state);
        assert_eq!(load(&path), state);

        std::fs::write(&path, br#"{"unread":3}"#).unwrap();
        let old = load(&path);
        assert_eq!(old.unread, 3);
        assert!(old.recent.is_empty());
    }
}
```

`NotifyState` needs `PartialEq` added to its derives for the assert. If `tempfile` is not already a dev-dependency of `ducktape-desktop`, follow the idiom the engine tests use for temp state paths instead (see `TestStatePath` in `engine.rs`) rather than adding a dependency.

- [ ] **Step 4: Run**: `ops/build-with.sh cargo test -p ducktape-desktop notify::state` → PASS.

- [ ] **Step 5: Commit** `feat(notify): persist a recent-notification ring in state.json`

---

### Task 2: Engine ring + Sink::item (Rust)

**Files:**
- Modify: `app/src-tauri/src/notify/engine.rs`

**Interfaces:**
- Consumes: `state::StoredNotification`, `NotifyState { unread, recent }`.
- Produces: `pub const RECENT_CAP: usize = 50`; `Engine::new(sink, state_path, recent: Arc<Mutex<VecDeque<StoredNotification>>>)` (third param NEW); `Sink::item(&self, item: &StoredNotification)` with default no-op body; ring is newest-first; every persisted write now includes the ring.

- [ ] **Step 1: Extend the Sink trait** (default no-op keeps every existing test sink compiling):

```rust
pub trait Sink: Send {
    fn present(&self, n: &Notification);
    fn badge(&self, unread: u32);
    /// A presented notification for the in-app bell (live dropdown update).
    fn item(&self, _item: &StoredNotification) {}
}
```

- [ ] **Step 2: Engine gains the shared ring.** Add `pub const RECENT_CAP: usize = 50;`, field `recent: Arc<Mutex<VecDeque<StoredNotification>>>`, and the third `Engine::new` parameter. `new` seeds the ring from the loaded state:

```rust
pub fn new(
    sink: S,
    state_path: PathBuf,
    recent: Arc<Mutex<VecDeque<StoredNotification>>>,
) -> Self {
    let loaded = state::load(&state_path);
    sink.badge(loaded.unread);
    *recent.lock().unwrap_or_else(PoisonError::into_inner) =
        loaded.recent.into_iter().collect();
    Self { sink, match_state: MatchState::default(), state_path, unread: loaded.unread, cursors: BTreeMap::new(), recent }
}
```

- [ ] **Step 3: Record on present.** In `handle()`, after `self.sink.present(&notification)`, build and push the stored item, then let the existing persist call (renamed, see step 4) write both:

```rust
let stored = StoredNotification {
    category: notification.category,
    title: notification.title.clone(),
    body: notification.body.clone(),
    channel_id: notification.channel_id.clone(),
    at: std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64),
};
{
    let mut recent = self.recent.lock().unwrap_or_else(PoisonError::into_inner);
    recent.push_front(stored.clone());
    recent.truncate(RECENT_CAP);
}
self.sink.item(&stored);
```

The unread increment/persist below it stays; drop the `if self.unread != previous_unread` skip (a new item must persist even on a saturated count) — persist unconditionally after a presented notification.

- [ ] **Step 4: Persist the ring.** Rename `persist_unread` → `persist` and write both fields:

```rust
fn persist(&self) {
    let recent = self
        .recent
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .cloned()
        .collect();
    state::save(&self.state_path, &state::NotifyState { unread: self.unread, recent });
}
```

Update both call sites (`handle`, `mark_seen`).

- [ ] **Step 5: Fix Engine::new callers in tests** — every `engine(...)` helper in `engine.rs`/`stream.rs` tests passes `Arc::default()` as the third argument.

- [ ] **Step 6: Add the ring test** (in `engine.rs` tests, alongside the existing ones — reuse their `TestStatePath`/capture-sink helpers):

```rust
#[test]
fn recent_ring_is_newest_first_capped_and_persisted() {
    let path = TestStatePath::new();
    let recent = Arc::new(Mutex::new(VecDeque::new()));
    let mut engine = /* construct as the existing tests do, but with recent.clone() */;
    for i in 0..(RECENT_CAP + 5) {
        /* feed one matching mention frame per iteration, i in the message id/body
           — copy the frame the existing mention test feeds */
    }
    let ring = recent.lock().unwrap();
    assert_eq!(ring.len(), RECENT_CAP);
    // newest first: the LAST fed frame is at the front
    assert!(ring[0].body.contains(&format!("{}", RECENT_CAP + 4)));
    drop(ring);
    // a fresh engine over the same state file reloads the ring
    let reloaded = Arc::new(Mutex::new(VecDeque::new()));
    let _engine2 = /* construct over the same path with reloaded.clone() */;
    assert_eq!(reloaded.lock().unwrap().len(), RECENT_CAP);
}
```

(Adapt frame construction from the existing mention test in the same file; the comment placeholders above are for the plan only — the implementation copies the concrete frame literal.)

- [ ] **Step 7: Run**: `ops/build-with.sh cargo test -p ducktape-desktop notify::` → all PASS.

- [ ] **Step 8: Commit** `feat(notify): engine keeps a shared recent ring and reports items to the sink`

---

### Task 3: notify_recent command, focus-backstop change, AppSink item event (Rust)

**Files:**
- Modify: `app/src-tauri/src/notify/mod.rs`
- Modify: `app/src-tauri/src/notify/present.rs`
- Modify: `app/src-tauri/src/main.rs:171` (command registration)

**Interfaces:**
- Consumes: `Engine::new(sink, path, recent)`, `StoredNotification`.
- Produces: command `notify_recent() -> Vec<StoredNotification>`; event `ducktape://notify-item` (payload = camelCase StoredNotification); OS focus no longer marks seen.

- [ ] **Step 1: NotifyHandles + init.** Add `pub recent: Arc<Mutex<VecDeque<state::StoredNotification>>>` to `NotifyHandles`. In `init`, create it before the engine and thread it through:

```rust
let recent = Arc::new(Mutex::new(std::collections::VecDeque::new()));
let engine = engine::Engine::new(present::AppSink(app.clone()), state_path, recent.clone());
```

and include `recent` in the `app.manage(NotifyHandles { .. })`.

- [ ] **Step 2: The behavior change.** In the focus backstop closure, DELETE the `if *focused { let _ = focus_cmds.send(Cmd::MarkSeen); }` block (and the now-unused `focus_cmds` clone). Rewrite the comment to say the backstop only floors `main_window_focused` for focus-suppression — seen-marking belongs to the bell dropdown (`notify_mark_seen` from the webview).

- [ ] **Step 3: The command** (below `notify_mark_seen`):

```rust
/// Recent presented notifications, newest first, for the in-app bell dropdown.
#[tauri::command]
pub fn notify_recent(
    state: tauri::State<'_, NotifyHandles>,
) -> Result<Vec<state::StoredNotification>, String> {
    Ok(state
        .recent
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .cloned()
        .collect())
}
```

Note: `state` the module vs `state` the parameter collide — use `crate::notify::state::StoredNotification` or a `use super::state::StoredNotification;` alias as the file's imports dictate.

- [ ] **Step 4: AppSink emits the live item** in `present.rs`:

```rust
fn item(&self, item: &StoredNotification) {
    // The sink cannot recover event delivery; log and continue (badge idiom).
    if let Err(err) = self.0.emit("ducktape://notify-item", item) {
        eprintln!("notify: could not emit notification item: {err}");
    }
}
```

- [ ] **Step 5: Register** `notify::notify_recent` in `main.rs`'s `generate_handler!` list next to the other two.

- [ ] **Step 6: Run**: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps` clean, `ops/build-with.sh cargo test -p ducktape-desktop notify::` PASS.

- [ ] **Step 7: Commit** `feat(notify): notify_recent command + live item events; focus no longer marks seen`

---

### Task 4: notify-client recent()/onItem() (TS)

**Files:**
- Modify: `app/src/domain/notify-client.ts`
- Test: `app/src/domain/notify-client.test.ts`

**Interfaces:**
- Produces: `NotifyItem { category: "mention"|"reply"|"huddle"|"run"|"forge"|"governance"; title: string; body: string; channelId: string | null; at: number }`; `recent(): Promise<NotifyItem[]>` ([] on web/failure); `onItem(cb: (item: NotifyItem) => void): Promise<() => void>`.

- [ ] **Step 1: Failing tests** (extend the existing describe, using its `tauriMocks`/`markTauri` scaffolding):

```typescript
it("recent returns the command payload and [] on failure", async () => {
  markTauri();
  invokeMock.mockResolvedValueOnce([
    { category: "mention", title: "t", body: "b", channelId: null, at: 1 },
  ]);
  const { recent } = await import("./notify-client");
  await expect(recent()).resolves.toHaveLength(1);
  expect(invokeMock).toHaveBeenCalledWith("notify_recent");

  invokeMock.mockRejectedValueOnce(new Error("nope"));
  await expect(recent()).resolves.toEqual([]);
});

it("onItem subscribes to the item event", async () => {
  markTauri();
  const unlisten = vi.fn();
  listenMock.mockResolvedValueOnce(unlisten);
  const cb = vi.fn();
  const { onItem } = await import("./notify-client");
  await onItem(cb);
  expect(listenMock).toHaveBeenCalledWith(
    "ducktape://notify-item",
    expect.any(Function),
  );
  const handler = listenMock.mock.calls[0][1];
  handler({ payload: { category: "reply", title: "t", body: "b", channelId: "c", at: 2 } });
  expect(cb).toHaveBeenCalledWith(
    expect.objectContaining({ category: "reply", channelId: "c" }),
  );
});
```

- [ ] **Step 2: Run** `cd app && bun run test src/domain/notify-client.test.ts` → the two new tests FAIL (no export).

- [ ] **Step 3: Implement** in `notify-client.ts` (mirror `markSeen`/`onUnread` shapes exactly, including `warnFailure`):

```typescript
export interface NotifyItem {
  category: "mention" | "reply" | "huddle" | "run" | "forge" | "governance";
  title: string;
  body: string;
  channelId: string | null;
  at: number;
}

export const recent = async (): Promise<NotifyItem[]> => {
  if (!isTauri()) return [];
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<NotifyItem[]>("notify_recent");
  } catch (error) {
    warnFailure("recent", "notification history is unavailable", error);
    return [];
  }
};

export const onItem = async (
  cb: (item: NotifyItem) => void,
): Promise<() => void> => {
  if (!isTauri()) return noop;
  try {
    const { listen } = await import("@tauri-apps/api/event");
    return await listen<NotifyItem>("ducktape://notify-item", (event) => {
      cb(event.payload);
    });
  } catch (error) {
    warnFailure("onItem", "live notification items are inactive", error);
    return noop;
  }
};
```

- [ ] **Step 4: Run** the file's tests → PASS.

- [ ] **Step 5: Commit** `feat(app): notify-client recent() and onItem()`

---

### Task 5: Provider stops marking seen on focus (TS)

**Files:**
- Modify: `app/src/console/store/DucktapeProvider.tsx:869-882`
- Test: `app/src/console/store/DucktapeProvider.test.tsx:~1276`

**Interfaces:**
- Consumes: nothing new. Produces: focus handler only flips `windowFocused`.

- [ ] **Step 1: Flip the existing test** — the focus test currently ends `expect(notifyMocks.markSeen).toHaveBeenCalled()`; change to `.not.toHaveBeenCalled()` and rename the `it` to say focus updates config without marking seen.

- [ ] **Step 2: Run** it → FAIL (markSeen still called).

- [ ] **Step 3: Implement** — in the focus effect delete `void notifyClient.markSeen();` so `onFocus` only calls `setWindowFocused(true)`. Update the effect's comment: seen-marking moved to the bell dropdown; this effect is now only the focus half of the config push.

- [ ] **Step 4: Run** the provider test file → PASS.

- [ ] **Step 5: Commit** `feat(app): window focus no longer clears the notification badge`

---

### Task 6: Bell icon, NotificationsBell component, TitleBar placement (TS)

**Files:**
- Modify: `app/src/console/components/Icon.tsx` (add `bell` glyph)
- Create: `app/src/console/layout/NotificationsBell.tsx`
- Modify: `app/src/console/layout/WindowFrame.tsx` (render in TitleBar right cell, before the status span)
- Test: `app/src/console/layout/NotificationsBell.test.tsx`

**Interfaces:**
- Consumes: `notifyClient.recent/onItem/onUnread/markSeen`, `NotifyItem`, `parseItemChannelId` from `domain/forge-client`, `useDucktape` actions `setScreen`/`selectChannel`, `isTauri`.
- Produces: `<NotificationsBell />` (self-contained; renders null on web).

- [ ] **Step 1: The glyph** — add to `PATHS` in `Icon.tsx` (stroke style like its neighbors):

```tsx
bell: (
  <>
    <path d="M6.5 15.5v-5a5.5 5.5 0 0 1 11 0v5l1.5 2H5z" />
    <path d="M10.5 18.5a1.5 1.5 0 0 0 3 0" />
  </>
),
```

- [ ] **Step 2: Failing component test** (`NotificationsBell.test.tsx`; mirror `PreferencesSection.test.tsx`'s ConsoleContext harness; `vi.mock("../../domain/notify-client")` with hoisted mocks for `recent`/`onItem`/`onUnread`/`markSeen`; mark tauri via `__TAURI_INTERNALS__` before render):

```tsx
it("shows the unread badge, marks seen on open, and navigates on item click", async () => {
  recentMock.mockResolvedValueOnce([
    { category: "mention", title: "Ping", body: "hey", channelId: "general", at: Date.now() },
  ]);
  let pushUnread: (n: number) => void = () => {};
  onUnreadMock.mockImplementation(async (cb) => { pushUnread = cb; return () => {}; });
  onItemMock.mockResolvedValue(() => {});

  const { setScreen, selectChannel } = renderBell(); // harness returns the action spies
  await act(async () => pushUnread(2));
  expect(screen.getByLabelText("Notifications")).toHaveTextContent("2");

  fireEvent.click(screen.getByLabelText("Notifications"));
  await waitFor(() => expect(markSeenMock).toHaveBeenCalled());

  fireEvent.click(await screen.findByText("Ping"));
  expect(setScreen).toHaveBeenCalledWith("chat");
  expect(selectChannel).toHaveBeenCalledWith("general");
});
```

Also a category-fallback case: an item `{ category: "run", channelId: null }` click → `setScreen("agent")` and no `selectChannel`; and a forge-item channel `forge:repo:4` → `setScreen("forge")`.

- [ ] **Step 3: Run** → FAIL (component missing).

- [ ] **Step 4: The component** — `NotificationsBell.tsx`:

```tsx
// The in-app notification surface: a bell in the title bar with the unread
// count, and a dropdown of the engine's recent ring. Opening the dropdown is
// what marks notifications seen (window focus deliberately does not — see the
// spec). Desktop-only: web builds have no notifier and render nothing.

import { useEffect, useRef, useState } from "react";

import { isTauri } from "../../domain/node-bootstrap";
import * as notifyClient from "../../domain/notify-client";
import type { NotifyItem } from "../../domain/notify-client";
import { parseItemChannelId } from "../../domain/forge-client";
import { Icon } from "../components/Icon";
import { accentVar, color, font, radius } from "../theme/tokens";
import { useDucktape } from "../store/use-ducktape";

const RECENT_CAP = 50;

// Category → rail screen when the item carries no channel.
const FALLBACK_SCREEN: Record<NotifyItem["category"], string> = {
  mention: "chat",
  reply: "chat",
  huddle: "chat",
  run: "agent",
  forge: "forge",
  governance: "governance",
};

const agoLabel = (at: number): string => {
  const s = Math.max(0, Math.floor((Date.now() - at) / 1000));
  if (s < 60) return "now";
  if (s < 3600) return `${Math.floor(s / 60)}m`;
  if (s < 86400) return `${Math.floor(s / 3600)}h`;
  return `${Math.floor(s / 86400)}d`;
};

export function NotificationsBell() {
  const { actions } = useDucktape();
  const [items, setItems] = useState<NotifyItem[]>([]);
  const [unread, setUnread] = useState(0);
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    const unlistens: Array<() => void> = [];
    void notifyClient.recent().then((initial) => {
      if (!cancelled) setItems(initial);
    });
    void notifyClient
      .onItem((item) => setItems((prev) => [item, ...prev].slice(0, RECENT_CAP)))
      .then((un) => (cancelled ? un() : unlistens.push(un)));
    void notifyClient
      .onUnread(setUnread)
      .then((un) => (cancelled ? un() : unlistens.push(un)));
    return () => {
      cancelled = true;
      unlistens.forEach((un) => un());
    };
  }, []);

  // Click-outside / Escape close (the CommentCard idiom).
  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!rootRef.current?.contains(event.target as Node)) setOpen(false);
    };
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open]);

  if (!isTauri()) return null;

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next) void notifyClient.markSeen();
  };

  const openItem = (item: NotifyItem) => {
    setOpen(false);
    if (item.channelId) {
      // A hidden forge-item channel is unroutable on the chat surface — the
      // provider's navigate listener makes the same detour.
      const forgeItem = parseItemChannelId(item.channelId);
      if (forgeItem) {
        actions.setScreen("forge");
        return;
      }
      actions.setScreen("chat");
      actions.selectChannel(item.channelId);
      return;
    }
    actions.setScreen(FALLBACK_SCREEN[item.category]);
  };

  return (
    <div ref={rootRef} style={{ position: "relative", flexShrink: 0 }}>
      <button
        onClick={toggle}
        aria-label="Notifications"
        title="Notifications"
        style={{
          all: "unset",
          boxSizing: "border-box",
          cursor: "pointer",
          display: "flex",
          alignItems: "center",
          gap: 4,
          padding: "3px 5px",
          borderRadius: radius.sm,
          color: unread > 0 ? color.ink : color.iconIdle,
        }}
      >
        <Icon name="bell" size={15} />
        {unread > 0 && (
          <span
            style={{
              font: `600 8.5px ${font.mono}`,
              color: "#fff",
              background: accentVar,
              borderRadius: 8,
              padding: "1px 5px",
            }}
          >
            {unread > 99 ? "99+" : unread}
          </span>
        )}
      </button>
      {open && (
        <div
          role="menu"
          aria-label="Recent notifications"
          style={{
            position: "absolute",
            top: 32,
            right: 0,
            width: 320,
            maxHeight: 400,
            overflowY: "auto",
            background: color.canvas,
            border: `1px solid ${color.border}`,
            borderRadius: radius.md,
            boxShadow: "0 8px 24px rgba(0,0,0,.18)",
            zIndex: 40,
            padding: 4,
          }}
        >
          {items.length === 0 ? (
            <div
              style={{
                padding: "18px 12px",
                textAlign: "center",
                font: `500 11px ${font.sans}`,
                color: color.muted,
              }}
            >
              No notifications
            </div>
          ) : (
            items.map((item, index) => (
              <button
                key={`${item.at}-${index}`}
                onClick={() => openItem(item)}
                style={{
                  all: "unset",
                  boxSizing: "border-box",
                  cursor: "pointer",
                  display: "block",
                  width: "100%",
                  padding: "7px 9px",
                  borderRadius: radius.sm,
                }}
                onMouseEnter={(e) => (e.currentTarget.style.background = color.sunken)}
                onMouseLeave={(e) => (e.currentTarget.style.background = "transparent")}
              >
                <div
                  style={{
                    display: "flex",
                    justifyContent: "space-between",
                    gap: 8,
                    font: `600 11px ${font.sans}`,
                    color: color.ink,
                  }}
                >
                  <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {item.title}
                  </span>
                  <span style={{ font: `500 9.5px ${font.mono}`, color: color.muted2, flexShrink: 0 }}>
                    {agoLabel(item.at)}
                  </span>
                </div>
                <div
                  style={{
                    marginTop: 2,
                    font: `400 10.5px ${font.sans}`,
                    color: color.muted,
                    display: "-webkit-box",
                    WebkitLineClamp: 2,
                    WebkitBoxOrient: "vertical",
                    overflow: "hidden",
                  }}
                >
                  {item.body}
                </div>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
```

Check `color.canvas` / `color.sunken` / `radius.sm` exist in `theme/tokens.ts` before using; substitute the file's actual token names for panel background, hover fill, and small radius if they differ.

- [ ] **Step 5: Place it** — in `WindowFrame.tsx` TitleBar right cell, first child before the status `<span>`: `<NotificationsBell />` (import at top). It must NOT carry `data-tauri-drag-region` (a control, like the search button).

- [ ] **Step 6: Run** `cd app && bun run test src/console/layout/NotificationsBell.test.tsx` → PASS, then the full `bun run test` → all green.

- [ ] **Step 7: Commit** `feat(app): notification bell + dropdown in the title bar`

---

### Task 7: Gates, live QA, PR

- [ ] **Step 1: Full gates** in the worktree: `ops/build-with.sh cargo clippy -p ducktape-desktop --tests --no-deps` clean; `ops/build-with.sh cargo test -p ducktape-desktop notify::`; `cd app && bun run test`.
- [ ] **Step 2: Live QA** via the `tauri-debug` skill (headless Xvfb recipe): launch the app, submit a mention op from a second identity via `/v1/submit` against the app's node, verify the bell badge increments, the dropdown lists the item, clicking navigates to the channel, and the badge clears on open (also confirm the window title `(N) Ducktape` badge from PR #433 clears on dropdown-open, not on focus).
- [ ] **Step 3: PR** against `dev` titled `feat(app): notification bell + dropdown in the title bar`, body summarizing the behavior change (seen moves from focus to dropdown-open) and QA evidence. Clean-context review, then merge on high confidence per repo rules.
