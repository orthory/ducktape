# Ducktape desktop

Native Chat + Pages client, with its UI declared in
`src/ui/app.ice` through [Ice](https://github.com/byeongsu-hong/ducktape-ui-lang).

```bash
cargo build -p node-bin
cargo run -p ducktape-app
```

The RPC defaults to `DUCKTAPE_NODE`, then `http://127.0.0.1:8844`, and remains
editable in the app. Chat + Pages hydrate over HTTP after the resumable
`module:chat` and `module:pages` WebSocket topics are active, then rehydrate on
committed changes. Writes require an encrypted v1 user key from
`DUCKTAPE_USER_KEY`, then `$DUCKTAPE_HOME/user.key`, then
`~/.ducktape/user.key`. Set `DUCKTAPE_BIN` when the `ducktape` CLI is neither
beside the app binary nor on `PATH`.

## Visual language

The canonical shared UI uses warm ink-on-paper neutrals and a sparse
terracotta brand role. Content stays opaque; functional chrome uses three
translucent tiers over the native-blurred window (thin rail/sidebar, regular
titlebar/popovers, sheet modals). Ice owns the opacity roles while blur remains
renderer-owned. Depth comes from surface steps and warm shadows.

| Token | Value | Use |
| --- | --- | --- |
| `bg` | `#fdfdfb` | the app canvas |
| `surface` | `#ffffff` | cards and controls |
| `muted_bg` | `#f6f5f2` | recessed wells and quiet regions |
| `sidebar` | `#fbfbf9` | opaque utility bars inside content |
| `elevated` | `#f3f2ef` | panels one notch above the canvas |
| `row_hover` | `#f8f7f3` | ordinary row hover |
| `fg` / `muted` | `#2c2b27` / `#6b6962` | warm ink and its secondary |
| `primary` | `#26251f` | neutral primary actions and focus |
| `brand` | `#a05a3c` | mentions, unread state, and action links |
| `glass_thin` | `rgba(253,252,250,.50)` | rail and sidebar |
| `glass_regular` | `rgba(253,252,250,.62)` | titlebar and floating chrome |
| `glass_sheet` | `rgba(253,252,250,.86)` | modal and sheet surfaces |

- Brand is sparse; danger, success, and warning colors are reserved for status.
- Selected navigation uses neutral `#ecebe6`; brand is for mentions, unread
  state, and action links.
- Hover changes fill, border, and foreground only; it never changes geometry.
- Reveal contextual row actions on hover and keep them visible while selected.
- Ice theme tokens are compile-time constants, so the app ships the shared
  default light palette.

## Design system

Shared color, shape, recipes, and components come from the pinned
`ducktape-ui` source vendored under `src/ui/ducktape-ui/`. The local `design`
crate owns only application font assets and the product type scale; drift
guards hold the Ice sources to both authorities.

- Faces: **Geist** (UI), **Geist Mono** (machine values, metadata, field
  labels, and badges).
  The files are embedded from `crates/design/assets/fonts/` at build time.
- Scale: 22 display · 20 screen title · 16 section · 14 pane header · 13.5
  body · 13 list · 12.5 caption · 12 machine value · 11/10.5 meta · 10 field
  label · 9.5 navigation · 9 badge.
- Frame: 1280×800 default, 40px titlebar, 74px permanent rail, 236px module
  sidebar, flexible content, and a 300px detail panel when present.
- Depth: cards stay paper-flat; floating bars/popovers use `0 3px 12px /.13`,
  brand tiles/toasts use `0 6px 18px /.22`, and modal sheets use
  `0 24px 60px /.30` with warm `#282622` ink.
