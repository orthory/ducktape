# OS-adaptive UI — design

**Date:** 2026-07-11
**Status:** Accepted
**Goal:** The desktop UI is tuned on macOS; make the Linux/Windows experience deliberate rather than accidental. Decide, per surface, whether the platforms should diverge — and fix the places where macOS-only affordances leak to other OSes.

## Audit result (what already varies correctly)

A full sweep of `app/src` + `app/src-tauri` found the platform seam largely in place:

- **Detection:** `isMacDesktop()` (`app/src/domain/node-bootstrap.ts:48`) on the frontend; `#[cfg(target_os)]` in Rust. No other detection needed.
- **Shortcut handlers** all accept `metaKey || ctrlKey` (`ConsoleShell.tsx:79`, `PagesView.tsx:123`) — Ctrl works today on Linux/Windows.
- **Fonts** are self-hosted (`@fontsource` Geist Sans/Mono, IBM Plex Sans KR) — identical rendering everywhere; `-apple-system` is only a dead-tail fallback.
- **Scrollbars:** `::-webkit-scrollbar` styling applies on WebKitGTK and WebView2 alike.
- **Traffic-light inset** (`WindowFrame.tsx:22`) is already `isMacDesktop() ? 69 : 0`.
- **Tray + popover + vibrancy** (`tray.rs`), **app menu** (`menu.rs`), **badge/tray-title** (`notify/present.rs`) are `cfg(target_os = "macos")`-gated no-ops elsewhere.
- **Media paths** feature-detect (`setSinkId`, DOMException names) rather than sniff the platform — correct.

## Defects to fix (this change)

1. **`⌘K` glyph leaks to non-mac** — `WindowFrame.tsx:46` (tooltip `Search (⌘K)`) and `:93` (visible kbd badge). The handler fires on Ctrl+K there, but the UI advertises a key that doesn't exist. Fix: label derives from `isMacDesktop()` → `⌘K` on macOS, `Ctrl K` elsewhere. One module-level const in `WindowFrame.tsx` (same pattern as `TRAFFIC_LIGHT_INSET`); no new helper module until a second consumer exists.
2. **Stale close-behavior doc** — `app/src-tauri/src/main.rs:8-11` claims "closing the console window only hides to the menu-bar app" universally, but close-to-hide is wired inside the mac-gated tray init (`tray.rs:97-105`). On Linux/Windows close quits the shell and stops the managed node. Fix the comment to state the divergence.

## Deliberate divergences (documented, not changed)

| Surface | macOS | Linux/Windows | Why keep the divergence |
|---|---|---|---|
| Window chrome | Overlay titlebar, traffic lights over in-app bar | Native decorations; in-app bar acts as a toolbar below them | `titleBarStyle`/`hiddenTitle` are mac-only Tauri fields, ignored elsewhere. Native decorations are platform-correct; the in-app bar carries real content (brand, search, status), not duplicate controls. |
| Close window | Hides to tray; node keeps running | Quits; managed node stops | No tray exists on Linux/Windows — intercepting close would strand an invisible app. Quit-on-close is the honest behavior there. |
| Tray / badge / tray title | Menu-bar app, dock badge | None | Badge unsupported on WebKitGTK; tray port is real work with no current Linux/Windows user base. |
| App menu | Native menu (exists chiefly to strip Cmd+W) | None | Non-mac has no default menu stealing Ctrl+W; webview provides native clipboard shortcuts in inputs. |

## Alternatives considered

- **Client-side decorations everywhere** (`decorations: false` + in-app min/max/close on Linux/Windows) for a mac-identical single bar: rejected — needs per-OS control ordering, resize-edge handling on GTK, snap interactions; heavy for zero current users.
- **Per-platform `tauri.*.conf.json`**: rejected — no config key currently needs to differ (mac-only keys are ignored on other OSes).
- **A `platform.ts` abstraction layer**: rejected — one boolean (`isMacDesktop`) gates everything today; an enum/layer is speculative.

## Ladder (when Linux/Windows become shipping targets)

1. Linux/Windows tray + close-to-tray parity (removes the close divergence).
2. Revisit CSD if design wants the single-bar look.
3. Promote the shortcut label to a shared helper when a second surface displays one.
