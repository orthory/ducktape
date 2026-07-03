# Fleet isolation finding + fix (verified)

- Date: 2026-07-03
- Context: reconciling the agent-driven QA work (PR #80) into the fleet dashboard
  (PR #82). This documents a **verified isolation bug** in the fleet bring-up and
  the fix that was proven to work, so whoever owns `ops/fleet.sh` can apply it
  collision-free (it couldn't be applied+verified here without disrupting a live
  fleet run).

## The bug: fleet tiles are NOT isolated — they all dial the shared `8844`

`ops/fleet.sh` isolates each instance's `$HOME` so `~/.ducktape` (the workspace
registry) and app-data don't collide. That isolation only *pays off if the app
boots into its desktop/LOCAL path* (reads its workspace registry, spawns its own
node). **It doesn't.** Verified read-only against the live fleet instance for
`feat-qa-multiwindow`:

```
isTauri: true,  badge: "REMOTE",  h 0
```

The app boots in **REMOTE** (web-client, `state.managed === false`) mode. In that
mode `node-bootstrap.ts::resolveNode` dials `VITE_DUCKTAPE_NODE_URL || http://127.0.0.1:8844`.
`fleet.sh up_one` sets no `VITE_DUCKTAPE_NODE_URL`, so **every REMOTE tile dials
the same `8844`** — the isolated `$HOME` is never consulted. The dashboard shows N
tiles that are all the *same* daemon's state, not per-worktree isolated nodes.

Why REMOTE and not LOCAL is unresolved: `__TAURI_INTERNALS__` is injected after the
webview mounts under headless `tauri dev` + external `devUrl`, but forcing the mount
to wait for it (a `main.tsx` deferred-mount guard) did **not** flip it to LOCAL even
with internals confirmed present at mount — so the cause is subtler than an
`isTauri()` timing race. LOCAL mode is a dead end for now; **the fix below makes
REMOTE mode correctly isolated instead.**

## The fix (verified working): give each instance its own node + dial it

Run an isolated node per instance and point the app at it via
`VITE_DUCKTAPE_NODE_URL`. Verified on an isolated instance: with a per-instance node
on `127.0.0.1:33105` and `VITE_DUCKTAPE_NODE_URL=http://127.0.0.1:33105`, the app
(REMOTE) reported `appHash 87f2739…` — **distinct** from `8844`'s `29dd3d78…`, i.e.
genuinely isolated. `invoke("workspace_active")` also read the instance's own
workspace, confirming the isolated `$HOME` is intact.

Concretely, `fleet.sh up_one` should, per instance (all inside the isolated `$HOME`):

1. **Stage a stable node binary** and pass it as `DUCKTAPE_NODE_BIN`. Do NOT point at
   `target/debug/ducktape-node` in a shared target dir — the tauri app build's
   `build.rs` overwrites that path with an empty placeholder, so spawning it fails
   with a baffling `Permission denied (os error 13)`. Copy a real node (prefer
   `target/release/ducktape-node`, which the debug app build doesn't clobber) to a
   stable path outside the target dir and use that.
2. **Found a solo workspace** by running the same verbs `workspace_create` uses —
   `ducktape-node init --name <id> --dir <dir> --listen 127.0.0.1:<p1> --advertised
   127.0.0.1:<p1> --http 127.0.0.1:<p2> --rpc 127.0.0.1:<p3>` then `ducktape-node
   keygen --out <dir>/identity.key` — the ports auto-allocated via `bind(:0)`.
3. **Start that node** detached: `ducktape-node --config <dir>/node.toml` (cwd
   `<dir>` so its relative `network.toml`/`identity.key`/`storage` resolve).
4. **Set `VITE_DUCKTAPE_NODE_URL=http://127.0.0.1:<p2>`** on the vite env so the
   REMOTE app dials *this* node.

`workspace_select`'s adopt-if-listening check means that if the app ever does boot
LOCAL later, it adopts this same node rather than double-spawning — so this fix is
forward-compatible with a future LOCAL-mode fix.

The reference implementation of steps 1–4 (dependency-free node) is `qa-instance.mjs`
on PR #80 (`stageNodeBin`, `seedWorkspace`, `startNode`); it can be ported to
`fleet.sh` bash or shelled out to.

## Dashboard note

The `fleet.json` "up" gate (`/tmp/tauri-mcp-<id>.sock` exists AND VNC port open) and
the TokenFile VNC feed are unaffected — this fix only changes *which node* each tile's
app talks to. No dashboard/VNC change is needed; the tiles just start showing isolated
per-worktree state instead of one shared daemon.
