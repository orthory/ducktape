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

## Material reference

The UI follows Apple's
[Materials](https://developer.apple.com/design/human-interface-guidelines/materials)
and [Liquid Glass](https://developer.apple.com/documentation/technologyoverviews/liquid-glass)
guidance. Glass is a functional layer for navigation and transient controls,
not a decoration for document or message content.

| Token | Opacity | Use |
| --- | ---: | --- |
| `bg` | 87% | content canvas |
| `surface` | 85% | inputs, composer, inline controls |
| `sidebar` | 86% | the full-height navigation pane |
| `popover` / `elevated` | 87% | alerts, menus, floating overlays |

- Keep the palette neutral gray; no gradients or colored accents.
- Use native window blur and let nearby content show through the material.
- Do not nest glass surfaces. Content rows stay flat until hover or selection.
- Hover changes fill, border, and foreground only; it never changes geometry.
- Reveal contextual row actions on hover and keep them visible while selected.

This first material pass is native window blur plus semantic translucency. It
does not claim optical lensing or refraction. Those effects require a
multi-surface renderer path; a gradient or alpha fill alone is not Liquid
Glass.
