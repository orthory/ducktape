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

The UI carries the Ducktape Console design source forward (the decommissioned
Tauri app's token system): warm ink-on-paper neutrals, a terracotta accent,
and FULLY OPAQUE surfaces — no window transparency, no blur, no glass.
Depth comes from one-notch surface steps and soft warm shadows, never from
translucency.

| Token | Value | Use |
| --- | --- | --- |
| `bg` | `#fcfcfc` | the app canvas the cards sit on |
| `surface` | `#f5f5f5` | inputs, composer, inline controls (recessed wells) |
| `sidebar` | `#f9f9f9` | the full-height navigation pane |
| `popover` | `#ffffff` | menus, alerts, floating overlays (paper + shadow) |
| `elevated` | `#efefef` | panels one notch above the canvas |
| `fg` / `muted` | `#2c2b27` / `#878787` | warm ink and its secondary |
| `primary` | `#a05a3c` | the terracotta accent (active nav, focus, unread) |

- Warm neutrals only; the accent is the single color voice.
- Hover changes fill, border, and foreground only; it never changes geometry.
- Reveal contextual row actions on hover and keep them visible while selected.
- Ice theme tokens are compile-time constants, so the app ships one palette;
  the design source's dark palette and the accent presets return with the
  module-UI runtime lane, where tokens become runtime values.

## Design system (`crates/design`)

Font identity, the type scale, and the depth recipes live in the `design`
crate — the app's `.ice` sources consume them through `style=` externs, the
shared `ice/kit.ice` components, and drift-guard tests that hold every
`size=` / `family=` literal to the crate's exports. Swap a face or a scale
step there, never inline in a view.

- Faces: **Geist** (UI), **Geist Mono** (data — hashes, seqs, diffs, logs;
  never a label), **Noto Sans KR** as the per-glyph CJK fallback. The files
  are embedded from `crates/design/assets/fonts/` at build time.
- Scale: 12 caption · 13 label · 14 body (the default text size) ·
  15 emphasis · 17 title · 20 display, plus 34 for a page's own title.
- Depth: `card` (paper + tight warm shadow), `raised` (floating paper),
  `well`/`inset` (recessed steps) — opaque surfaces always, shadow never
  translucency.
