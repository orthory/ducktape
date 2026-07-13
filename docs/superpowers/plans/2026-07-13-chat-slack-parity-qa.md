# Chat ⇄ Slack parity — QA ledger (2026-07-13)

Companion to the `feat/chat-slack-polish` PR. Part 1 records what that PR
shipped and how it was verified; part 2 is the ranked backlog from a full
Slack-equivalency review of the chat surface — the "initial QA tasks."

## 1. Shipped in `feat/chat-slack-polish`

| Fix | Verification |
|-----|--------------|
| Hover action bar / overflow menu / emoji picker anchor at the pane's right edge (880px content cap removed; rows span the pane) | live fleet instance, real-pixel screenshots |
| Code fences wrap (`pre-wrap`) instead of scrolling sideways; unbroken tokens wrap mid-token | live + `ChatView.test.tsx` pins the wrap contract |
| Body/composer 15px (was 13.5), full scale lifted (names, timestamps, reactions, menus, rail) | live screenshots |
| Inline `` `code` `` chips; fenced blocks get border + shiki syntax highlighting (reuses the forge viewer's highlighter, theme-aware `.code-tok`) | live (rust fence highlighted) + `rich-text.test.tsx` |
| Quotes render upright with a 3px bar (was italic) | live |
| Channel rail + thread panel drag-resizable (`PanelResizer`); width rides `--chat-rail-w`/`--chat-thread-w` CSS vars, persists in localStorage, double-click resets; huddle dock tracks the rail var | live drag via real pointer + localStorage inspected + `PanelResizer.test.tsx` |

Live QA: fleet instance from the worktree, full onboarding, channel + 8
messages exercising every block/mark kind, thread reply, reaction round-trip
(chip appears in lane AND thread panel), both panels resized by real pointer
drag, persisted widths confirmed in localStorage.

## 2. Ranked backlog — initial QA tasks

From a full-code Slack-parity review (agent-swept, findings verified against
source). Severity = what a daily Slack user hits.

### Blockers — the "catch-up spine" (all client-side, no wire change)

1. **Unread state** — no per-channel unread badge, no "New messages" divider,
   no jump-to-unread. Shape: local last-read map (channel id → seq) in
   localStorage; unread = `channel.head_seq − lastRead`; rail badge + stream
   divider + jump button. (Cross-device read cursors would need a chat-module
   op — defer.)
2. **No scroll-back through history** — channel loads newest 256
   (`MAX_QUERY_LIMIT`, domain/chat-client.ts) and scrolling up dead-ends.
   Shape: scrollTop-near-zero branch in `MessageList`'s `onScroll` fetching an
   older slice (extend `messagesAround`), prepend preserving scrollHeight.
3. **Scroll position not restored** — every channel switch pins to bottom
   (`pinnedRef` forced true, ChatView). Shape: persist per-channel scrollTop or
   reuse the last-read seq from (1).
4. **Direct messages** — no DM concept at all. Product-scale: model as a
   2-member members-only channel or new channel kind; touches the Rust chat
   module. Needs its own design pass.

### Irritants

5. **New-message indicator** — scrolled up + new message = silent; add a
   floating "N new ↓" pill when unpinned and messages grow.
6. **Hover-only actions (a11y)** — actions + grouped-row timestamps are
   mouse-hover only; rows unfocusable. Keep the bar mounted (visually hidden
   until hover/`:focus-within`), add roles.
7. **Draft bleeds across channels** — one global draft in ChatView follows you
   between channels and dies on restart. Key drafts by channel id.
8. **Up-arrow to edit last message** — not wired; add ArrowUp-on-empty-draft →
   edit own last message.
9. **Edit box is a bare textarea** — editing loses mention typeahead, toolbar,
   paste-upload. Reuse `<Composer>` for edit.
10. **Reaction "who reacted"** — chips show counts only; reactor identities are
    already in `reaction.reactors` — resolve through `authorName` into the
    chip title.
11. **Attachments are paste-only** — add paperclip file-picker + drag-and-drop,
    both reusing `attachFiles`.
12. **Markdown coverage** — no lists / strikethrough / headers. Strike = new
    mark, lists = new block kinds → Rust chat module + `chat-input`/`rich-text`
    round-trip. Batch as one wire change (no-backcompat: in-place update).
13. **`:shortcode:` emoji typeahead** — add a `:`-token detector mirroring
    `mentionTokenAt`; expand to unicode on pick.
14. **Pinned messages** — `Channel.pinned` already on the wire but unused; add
    Pin overflow action + header pinned view.
15. **Channel topic/description** — no topic field; needs chat-module
    `SetTopic` op + header edit/display.
16. **Member list for open channels** — roster/count only exists for
    members-only channels.

### Nice-to-have

17. In-channel search (the `search` query already accepts `channelId`; the ⌘K
    palette just doesn't pass it).
18. Formatting shortcuts — Cmd/Ctrl+B/I → existing `wrap()` helper.
19. Link unfurls (keep view-local if ever; consensus-replicated unfurls are a
    deliberate non-goal).
20. Broadcast mentions (@here/@channel) — new mention kind → module + notifier.
21. Self-mention row highlight.
22. Channel-slice loading skeleton.

### Infra repairs found during QA

23. **`npm ci` is broken on a clean dev checkout** — `app/package-lock.json`
    is out of sync with `package.json` (fleet/shiki/noble entries missing), and
    `@byeongsu-hong/tauri-agent-plugin` pins `peerOptional jsdom@^26` vs the
    root's jsdom 29 → fresh installs need `--legacy-peer-deps` plus a manual
    `@testing-library/dom` install. Fix: regenerate the lockfile, declare
    `@testing-library/dom` as a devDep, bump the plugin's peer range.
24. **QA hosts render emoji as tofu** — headless Xvfb has no color-emoji font,
    so quick-reacts/chips show boxes in screenshots. Install a color-emoji font
    into the fleet image (or note it as a known artifact in the qa skill).

### Explicitly at/beyond parity (don't re-litigate)

Finalization marks + explorer deep-links, copy link/reference, agent
mentions/Ask-agent, mention + page-ref typeaheads, duckfs-confined attachments
(security parity+), #tag catalog/filter, members-only management, huddles,
empty states, day dividers + grouping + IME-safe input.
