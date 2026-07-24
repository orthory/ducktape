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
