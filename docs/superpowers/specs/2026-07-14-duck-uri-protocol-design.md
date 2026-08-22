# duck:// URI Protocol (v1)

2026-07-14. Status: approved for implementation (single PR against `dev`).
2026-08-22: the module table lives in the iced app (`app/src/backend/duck_uri.rs`,
`classify_duck_link`); the open plane is `open_message_link` (`handlers/chat.ice`);
`forge` gains the `/<repo>/blob/<path>[@<rev>]` row and `![…]` image embeds resolve
through the table in the forge reader's Markdown.

## Problem

`duck://` already has four consumers speaking three half-different grammars:
the chat tokenizer hardcodes `page|files` (`app/src/console/views/chat/duck-ref.ts`),
the agent runtime mirrors that pair in Rust (`crates/apps/runs/src/inject.rs`),
duckfs mints ad-hoc `duck://files<path>@<snapshot>#L<line>` / `duck://memory<path>`
evidence URIs (`crates/duckfs/core/src/wire.rs`), and the browser resolves
`duck://<name>.duck/...` gateway hosts (`app/src/domain/duck-browser.ts`).
Nothing defines the namespace, so `duck://forge/...` (a PR link in chat) renders
as dead text and adding a module means three-file regex surgery.

This spec protocolizes the scheme: one grammar, one module table, one place to
add a module. The app is a super-app; `duck://` becomes its deep-link fabric.

## Grammar

```
duck-uri     = "duck://" authority path [ "@" rev ] [ "#" fragment ]
authority    = module | gateway-host
module       = [a-z][a-z0-9-]*          ; single label, NO dots
gateway-host = contains "."             ; must end ".duck" — the gateway plane
```

Two authority planes, split by the presence of a dot:

- **Module plane** (this spec): `duck://<module>/<path>` names an in-app
  resource. Resolved client-side by the module table below.
- **Gateway plane** (unchanged): `duck://<name>.duck/...` names published
  network content, resolved by `duck-browser.ts` / gateway v2. This spec only
  *reserves* the dotted namespace; it does not touch that code path.

The chat/comment **reference form** is unchanged from #550: standard markdown
link/image syntax over a duck URI — `[label](duck://…)` / `![label](duck://…)`.
Bare URIs stay literal text; refs ride the wire as PLAIN markdown text (the
chat parser marks only `https?://`), so the agent runtime sees the same bytes.

## Module table (v1)

| module | path | chip face | open (deep link) | agent inject |
|---|---|---|---|---|
| `page` | `/<id>` (one segment) | live store title, else raw id | `openPage` + `setScreen("pages")` | page subtree (current) |
| `files` | `/shared/attachments/<dir>/<name>` | filename chip; `![…]` image embeds | download / inline preview (current) | committed text (current) |
| `forge` | `/<repo>` or `/<repo>/<n>` (`n`, `seq` = decimal digits); `#<seq>` anchors a Discussion message; `/<repo>/blob/<path>[@<rev>]` names a committed file (`rev` = oid or branch, default head; no dot-segments) | `<repo>` or `<repo>#<n>`; a blob `![…]` embeds the picture | `openForgeItem({repo, number, messageSeq})` — the existing one-shot `forgeFocus` hand-off; a blob opens the Code tab's reader on that file (at the tree's head — the browser does not pin `@rev`) | none (nav-only) |
| `channel` | `/<id>` (ids may contain `:`); `#<seq>` anchors a message | `#<name>` from the store, else raw id | `selectChannel` / `focusMessage(id, seq)` (scroll+flash); `forge:<repo>:<n>` ids reroute to the forge item via the existing `forgeItemTarget` helper | none (nav-only) |

Reserved names: `memory` (agent-side evidence URIs, Rust-only), every dotted
host (gateway plane). An unknown module or malformed path is NOT an error in
chat — the ref stays literal text, losslessly (existing behavior); the browser
address bar reports it as unresolvable.

## Behavior planes

1. **Reference plane** — the chat/comment tokenizer (`splitDuckRefs`) matches
   the markdown link form over ANY module-plane URI, then validates through the
   module table. Valid → typed ref segment; invalid → literal text.
2. **Open plane (deep links)** — one adapter `openDuckRef(ref, actions)` maps a
   ref onto the store's EXISTING navigation vocabulary: `openPage`,
   `openForgeItem`, `selectChannel`, `focusMessage`. These are the same targets
   the desktop-notification `NavigateTarget` patches — the protocol adds no new
   navigation machinery, only a textual address for what already exists.
3. **Injection plane (agent)** — per-module opt-in. v1 injects exactly what
   ships today: `page` (subtree) and `files` (committed text). **Rust is
   untouched**: the fuzz-verified invariant "Rust refs ⊆ TS refs — the agent
   never over-reads" is preserved by only widening the TS side with
   navigation-only modules. Forge/channel refs reach the agent as literal
   markdown; it can act on them through tools.

## Canonical-face rule

A chip's visible face comes from canonical/store data (`page` title, `#channel`
name, `repo#n`), never from the markdown label. The label is decorative — this
is the page-chip precedent and the anti-spoof guard: `[not the pr](duck://forge/x/1)`
renders as `x#1`.

## Architecture

- **`app/src/domain/duck-uri.ts`** (new) — the protocol core: generic parse
  (authority/path/`@rev`/`#fragment`, plane split) + `classifyDuckRef` (module
  table) + the `DuckRef` discriminated union. Single source of truth; no
  registry machinery — adding a module is one union variant, one classify row,
  one chip, one `openDuckRef` arm, one ADR table row.
- **`chat/duck-ref.ts`** — keeps the markdown tokenizer (`splitDuckRefs`) and
  the composer builders; classification delegates to the domain core.
- **`chat/rich-text.tsx`** — `ForgeRefChip` + `ChannelRefChip` alongside the
  existing page/file chips; every chip click routes through `openDuckRef`.
- **`store` wiring** — `openDuckRef` is a thin function over `ConsoleActions`;
  no new state.
- **`BrowserView`** — a module-plane URI typed in the address bar hands off to
  `openDuckRef` (in-app navigation) instead of erroring; dotted hosts keep
  resolving through the gateway plane as today.
- **`docs/adr/2026-07-14-duck-uri-protocol.mdx`** — the normative grammar +
  module table + registration rule.

## Security invariants (carried, not new)

- `files` confinement byte-identical to today: exactly
  `/shared/attachments/<dir>/<name>`, two segments, no dot-segments — in both
  the TS tokenizer and the untouched Rust injector.
- Widening the chat grammar must not widen agent reads (injection untouched).
- Every ref validates before it chips; malformed refs degrade to literal text.
- The open plane is client-side navigation only — it grants no read the data
  layer wouldn't already serve; a channel/repo the node won't return renders
  its screen empty, same as manual navigation.

## Testing

- `domain/duck-uri.test.ts` (new): plane split (dotted host → gateway/null),
  each module row, `@rev`/`#fragment` parsing, files-confinement cases ported.
- `chat/duck-ref.test.ts`: existing cases stay green; forge/channel tokenize;
  unknown module stays literal; lossless round-trip property extended.
- `chat/rich-text.test.tsx`: new chips render canonical faces and dispatch the
  right actions.
- `BrowserView.test.tsx`: module-URI handoff.

## Out of scope (follow-ups, deliberately)

- Composer typeahead for forge/channel refs (page `[[` typeahead precedent).
- Chipping gateway-plane (`*.duck`) links in chat → open the browser screen.
- Agent injection for forge/channel (needs a product decision on what content
  a nav ref should pull).
- OS-level `duck://` URL-scheme registration (outside-the-app deep links).
