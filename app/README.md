# Ducktape desktop

Minimal native Chat + Pages client, with its complete UI declared in
`src/ui/app.ice` through [Ice](https://github.com/byeongsu-hong/ducktape-ui-lang).

```bash
cargo build -p node-bin
cargo run -p ducktape-app
```

The RPC defaults to `DUCKTAPE_NODE`, then `http://127.0.0.1:8844`, and remains
editable in the app. The node tip is polled once per second and Chat + Pages
reload only when the block height advances. Writes are signed by `DUCKTAPE_USER_KEY`, then
`$DUCKTAPE_HOME/user.key`, then `~/.ducktape/user.key`. Set `DUCKTAPE_BIN` when
the `ducktape` CLI is neither beside the app binary nor on `PATH`.
