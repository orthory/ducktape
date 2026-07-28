# Chat tag system — design

Date: 2026-07-07. Status: approved for implementation (autonomous tailored call).

## Goal

Users can tag chat messages with `#hashtags` and filter a channel (or the whole
workspace) by tag. Tags are discoverable (per-channel tag list with counts).

## Decision: hashtags in text, indexed node-locally. Zero consensus impact.

Three approaches considered:

1. **Consensus-side structured tags** (new `ChatMsg` op / new `MessageHead`
   field): consensus-breaking → dark-ship + `Env::protocol_version` gating or
   lockstep flag day. Heavy machinery for a labeling feature. Rejected.
2. **Tagging plane** (`crates/system/tagging`): it routes `EntityRef`s between
   modules (engagement), it is not a label store. Wrong shape. Rejected.
3. **Hashtags parsed from message text, indexed in the existing derived chat
   search view** (`crates/apps/chat/src/index.rs`). The indexer layer is
   documented as never part of any `root()`/root-hash → node-local, no
   consensus impact, no upgrade gate. Message text already carries the tags,
   so history is retroactively taggable via the existing
   `rebuild_from_state` path. **Chosen.**

## Tag syntax + normalization

- A tag is `#` followed by 1..=64 chars of Unicode letters/digits/`_`/`-`.
  Terminated by anything else.
- `#` must be at start-of-text or preceded by whitespace/punctuation (don't
  tag `foo#bar` or URLs' fragments).
- Extracted from `Paragraph` and `Quote` blocks only — never `Code` blocks or
  `Span`s carrying a `Mark::Link`.
- Index key = NFC-normalized, lowercased form. Display form = as typed.
- Cap: at most 16 distinct tags indexed per message (mirror
  `MAX_TAGS_PER_EVENT`-style caps elsewhere).

## Index changes (`crates/apps/chat/src/index.rs`)

Follow the existing `tok/` inverted-index pattern exactly:

- Postings: `tag/{label}/{channel_id}/{seq}` — written in `index_op` on
  `PostMessage`; on `EditMessage` diff old vs new tag sets (remove stale,
  add new); on `DeleteMessage` remove that message's tag postings.
- Catalog: `tagcat/{channel_id}/{label}` → `{ count, last_seq }` maintained
  incrementally (count of live messages carrying the tag in that channel).
- `rebuild_from_state` must produce identical postings/catalog (add a test
  asserting fold-vs-rebuild parity for tags, mirroring any existing parity
  test for `tok/`).

New view queries (node-local wire, no consensus concern; follow
`ChatViewQuery::Search` shape):

- `ChatViewQuery::Tags { channel_id: Option<String>, limit }` →
  `ChatViewReply::Tags(Vec<TagRow>)` where
  `TagRow { tag, count, last_seq }`, ordered by count desc then tag.
  `channel_id: None` aggregates across channels.
- `ChatViewQuery::TagSearch { tag, channel_id: Option<String>, limit }` →
  existing `ChatViewReply::Hits(Vec<MsgRow>)`, newest-first, clamped to
  `MAX_SEARCH_LIMIT` like `Search`.

## App changes

- `app/src/domain/chat-client.ts`: add `tags(channelId?)` and
  `tagSearch(tag, channelId?)` typed wrappers over `transport.view` (mirror
  `searchMessages`).
- `MessageItem.tsx` already tints plain-text `#token`s — make them
  clickable: clicking sets the channel's active tag filter.
- `ChatView.tsx`: tag filter state (store or local view state). When a tag
  filter is active: show a dismissible filter bar ("#tag — N messages ✕")
  above the message list and render the `tagSearch` hits (read-only rows,
  same `MsgRow` rendering approach the search surface uses) instead of the
  live latest-256 slice. Clearing the filter returns to the live view.
- Channel header: a small tag chip row or dropdown listing the channel's top
  tags (from `Tags` query) as entry point. Keep minimal; autocomplete in the
  composer is explicitly deferred.
- Composer: no changes required (tags are plain text).

## Non-goals / deferred

- Consensus-side structured tags, tag-based hooks/automations triggers,
  composer autocomplete, tag renaming/merging, read-side ACLs.

## Testing

- Rust: tag extraction unit tests (non-ASCII text, `_`/`-`, code-block exclusion,
  mid-word `#` rejection, 16-tag cap, NFC/lowercase normalization); fold
  tests for post/edit/delete; fold-vs-rebuild parity; `Tags`/`TagSearch`
  query tests incl. channel scoping and limit clamp.
- App: typecheck + existing lint/test suite; manual verify via tauri-debug
  is optional (headless box), not required for merge.
