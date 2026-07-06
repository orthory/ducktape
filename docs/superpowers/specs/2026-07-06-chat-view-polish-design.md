# Chat View Polish — Design

Date: 2026-07-06 · Branch: `feat/chat-view-polish` (from `origin/dev`)

QA'd the live app over the tauri-debug socket + a 4-lens audit of the chat
surface. Confirmed the user's three complaints and one latent bug. Light theme
only (no dark mode), inline-style + token system, Slack-style stream.

## Problems (confirmed)

1. **Checkmarks everywhere.** `FinalizationMark` paints a persistent green ✓ on
   every finalized op. Every chat message accrues one (8 msgs → 8 ✓), and on
   grouped continuation rows the ✓ takes the avatar/time gutter, making a column
   of checks down the left margin. A consensus-inclusion badge is meaningful for
   *deliberate* actions (a vote, a save) but is pure noise in chat, where every
   message is committed by definition.
2. **Broken tooltip.** The mark's hover tooltip is `left:50%; translateX(-50%)`
   and is clipped on its left edge by ancestor `overflow:hidden`
   (MessageList/ChatView), mangling the text ("d at height 55…", "o view in
   explorer") and dumping a raw 64-hex hash.
3. **Too empty.** Sparse stream: thin left-aligned column in a wide pane, a
   one-line grey in-stream empty state (vs. the much nicer `EmptyChannelState`),
   no day dividers at all (genesis-relative time → `isWallClock` false), flat
   rhythm between author groups.
4. **Latent bug — agents invisible.** `chat-helpers.isAgentAuthor` tests
   `"Agent" in author` (capital) but the wire variant is lowercase
   `{ agent: {...} }`, so it is *always false*: agents never get the AGENT pill
   or the square dark avatar — removing the stream's only high-contrast element.

## Changes

### A. Finalization marks (chat) — kill the spam, keep the signal
- In `MessageItem`, render `FinalizationMark` **only for `pending`/`failed`**;
  finalized renders nothing. Pending dot (your just-sent message) and the failure
  cross remain. The grouped-row gutter falls back to the hover-timestamp for
  finalized/no-op rows (no more check column).
- Preserve the explorer jump by moving it into the row's **overflow menu**:
  an "Open in explorer" entry when the op has a known inclusion height (reads the
  store's `openExplorerAt` via `ConsoleContext`). More discoverable than a 11px ✓.

### B. Finalization tooltip (global, shared component)
- Make the tooltip **edge-aware / non-clipping**: render via a portal to
  `document.body` with `position:fixed` computed from the mark's bounding rect,
  clamped to the viewport. Same content/roles (tests stay green), just escapes
  the `overflow:hidden` ancestors. Benefits every surface that uses the mark.

### C. `isAgentAuthor` fix
- `"Agent" in author` → `"agent" in author`. Agents regain the pill + square
  avatar (correctness *and* visual differentiation).

### D. Empty states + structure (the "too empty" fix)
- **In-stream empty state**: replace the one-liner with a centered icon-tile
  block matching `EmptyChannelState` (chat glyph in a `sunken` square + title +
  hint).
- **Beginning-of-channel marker**: a Slack-style hero above the first message —
  hash tile + "This is the beginning of #channel." + policy note. Adds top-of-
  pane substance and structure that survives the missing day dividers.
- **Thread panel empty state**: same upgraded treatment, scaled to the panel.
- **Rhythm**: a bit more separation between author groups so the stream reads as
  organized blocks, not floating text. Subtle, token-consistent.

## Non-goals
- No dark mode (doesn't exist). No global change to `FinalizationMark`'s
  finalized rendering in non-chat views (a vote/save badge is legitimate there).
- No faked wall-clock timestamps.

## Verify
- Typecheck + chat tests. Drive a data-connected web preview (worktree vite →
  live node) with Playwright: post a message (pending dot → clean), confirm no
  persistent checks, open a thread, trigger the overflow menu, screenshot.
- Adversarial review workflow on the diff before commit/push.
