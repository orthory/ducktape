# OS-adaptive UI — design

**Date:** 2026-07-11
**Status:** Accepted
**Goal:** The desktop UI is tuned on macOS; make the Linux/Windows experience deliberate rather than accidental. Decide, per surface, whether the platforms should diverge — and fix the places where macOS-only affordances leak to other OSes.

## Audit result (what already varies correctly)

A full sweep of `app/src` + `app/src-tauri` found the platform seam largely in place:

- **Detection:** `isMacDesktop()` (`app/src/domain/node-bootstrap.ts:48`) on the frontend; `#[cfg(target_os)]` in Rust. No other detection needed.
- **Shortcut handlers** all accept `metaKey || ctrlKey` (`ConsoleShell.tsx:79`, `PagesView.tsx:123`) — Ctrl works today on Linux/Windows.
- **Fonts** are self-hosted (`@fontsource` Geist Sans/Mono) — identical rendering everywhere; `-apple-system` is only a dead-tail fallback.
- **Scrollbars:** `::-webkit-scrollbar` styling applies on WebKitGTK and WebView2 alike.
- **Traffic-light inset** (`WindowFrame.tsx:22`) is already `isMacDesktop() ? 69 : 0`.
- **Tray + popover + vibrancy** (`tray.rs`), **app menu** (`menu.rs`), **badge/tray-title** (`notify/present.rs`) are `cfg(target_os = "macos")`-gated no-ops elsewhere.
- **Media paths** feature-detect (`setSinkId`, DOMException names) rather than sniff the platform — correct.

## Defects to fix (this change)

1. **`⌘K` glyph leaks to non-mac** — `WindowFrame.tsx:46` (tooltip `Search (⌘K)`) and `:93` (visible kbd badge). The handler fires on Ctrl+K there, but the UI advertises a key that doesn't exist. Fix: label derives from `isMacDesktop()` → `⌘K` on macOS, `Ctrl K` elsewhere. One module-level const in `WindowFrame.tsx` (same pattern as `TRAFFIC_LIGHT_INSET`); no new helper module until a second consumer exists.
2. **Stale close-behavior doc** — `app/src-tauri/src/main.rs:8-11` claims "closing the console window only hides to the menu-bar app" universally, but close-to-hide is wired inside the mac-gated tray init (`tray.rs:97-105`). On Linux/Windows close quits the shell and stops the managed node. Fix the comment to state the divergence.
3. **The header only accommodates mac traffic lights** (user call, superseding the first draft's "keep native decorations"). With native decorations, Linux/Windows got a second title bar above the in-app one — a visibly second-class chrome. Fix: single-bar chrome everywhere. Off-mac the shell drops native decorations at setup (`main.rs`), and the title bar hosts in-app minimize/maximize/close (`WindowChrome.tsx`, right cell of the bar); invisible 4px edge strips drive WM resize via `startResizeDragging` (WebKitGTK gives an undecorated window no resize border). Dragging already works via `data-tauri-drag-region`; capabilities gain `start-resize-dragging`/`minimize`/`toggle-maximize`.

## Deliberate divergences (documented, not changed)

| Surface | macOS | Linux/Windows | Why keep the divergence |
|---|---|---|---|
| Window chrome | Overlay titlebar, native traffic lights over the in-app bar (left) | Undecorated window; in-app controls in the bar's right cell + edge resize strips | Same single-bar look; only the control *mechanism* differs, since `titleBarStyle: Overlay` is a mac-only Tauri field. Controls sit right per Windows/GNOME convention. |
| Close window | Hides to tray; node keeps running | Quits; managed node stops | No tray exists on Linux/Windows — intercepting close would strand an invisible app. Quit-on-close is the honest behavior there (the in-app close button routes through the same `CloseRequested` path). |
| Tray / badge / tray title | Menu-bar app, dock badge | None | Badge unsupported on WebKitGTK; tray port is real work with no current Linux/Windows user base. |
| App menu | Native menu (exists chiefly to strip Cmd+W) | None | Non-mac has no default menu stealing Ctrl+W; webview provides native clipboard shortcuts in inputs. |

## Alternatives considered

- **Native decorations on Linux/Windows** (in-app bar as a toolbar below the OS title bar): the first draft's pick, vetoed by the user — the doubled chrome is exactly the "second-class off-mac" feel this work exists to remove.
- **Per-platform `tauri.*.conf.json`** for `decorations: false`: rejected — JSON merge patch replaces the `windows` array wholesale, so both files would duplicate the whole main-window object (drift hazard). A cfg-gated `set_decorations(false)` at setup is one line; the cost is one decorated first paint on launch.
- **A `platform.ts` abstraction layer**: rejected — one boolean (`isMacDesktop`) gates everything today; an enum/layer is speculative.

## Ladder (when Linux/Windows usage grows)

1. Linux/Windows tray + close-to-tray parity (removes the close divergence).
2. Maximized-state polish: restore glyph on the maximize button, suppress edge strips while maximized.
3. Promote the shortcut label to a shared helper when a second surface displays one.
4. Per-platform tauri conf files if the decorated first-paint flash matters.
