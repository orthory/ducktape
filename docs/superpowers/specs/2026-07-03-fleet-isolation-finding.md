# Fleet isolation finding (UNSOLVED — needs app-side investigation)

- Date: 2026-07-03
- Context: reconciling agent-driven QA into the fleet dashboard (PR #82 / #85).
- **Status: the fleet's per-tile isolation is broken, and I could NOT fix it.**
  This documents what's wrong, everything I tried that did **not** work, and where
  the real problem is (the app's headless boot, not `fleet.sh` plumbing) — so the
  next person doesn't repeat the dead ends.

## The bug (confirmed)

`fleet.sh` isolates each instance's `$HOME`, so each app has its own `~/.ducktape`.
That only pays off if the app boots into its **desktop/LOCAL** path and connects
its own workspace node. It does **not**: verified read-only on a live tile,

```
isTauri: true,  badge: "REMOTE"  (state.managed === false)
```

In the web/REMOTE path the app dials `VITE_DUCKTAPE_NODE_URL || http://127.0.0.1:8844`,
so absent that var **every tile talks to the shared 8844** — the isolated `$HOME`
is never used and all tiles show one node.

**Definitive check:** created a chat channel on a per-instance node (8851) via
`/v1/submit`, reloaded the app — it did NOT appear (`No channels yet`). The app is
not bound to the isolated node.

## What did NOT work (dead ends — don't retry these)

1. **`VITE_DUCKTAPE_NODE_URL` in the app's process env** — Vite only surfaces
   `VITE_*` from `.env` files, not `process.env`, so it never reached the client.
2. **`vite.config.ts` `define` for `import.meta.env.VITE_DUCKTAPE_NODE_URL`** — Vite
   handles `import.meta.env` specially; the `define` did not take. App still on 8844.
3. **`app/.env.local` with the var** — app still did not bind to the isolated node.
4. **Deferring the React mount until `__TAURI_INTERNALS__` is injected** (a `main.tsx`
   guard) — did NOT flip REMOTE→LOCAL even with internals confirmed present at mount.

## The real problem is app-side, not fleet.sh

The contradiction is the crux: **`isTauri()` returns true, yet the app is
`managed:false` (REMOTE)**. Per `DucktapeProvider`'s boot effect, `isTauri()===true`
should take the desktop branch → `activeWorkspace()` → `connectActive` →
`workspace_select` → `managed:true` (LOCAL). It doesn't. So the desktop/workspace
connect is failing or being bypassed *silently* under headless `tauri dev` served
from an external `devUrl`. Compounding it, the headless app's data layer looks
non-functional (height stuck at the initial 0, no channels rendered from any node,
`/v1/ws` not live), which makes UI-level verification unreliable.

**Correction to an earlier draft of this doc:** it claimed the
`VITE_DUCKTAPE_NODE_URL` fix was "verified (distinct appHash from 8844)." That was
wrong — the distinct appHash was read from the *node* via curl; the *app's* actual
connection was never confirmed, and later testing shows the app does not bind to the
isolated node. No isolation fix is verified.

## Recommended next step (for whoever owns the app)

Investigate, in the headless `tauri dev` + external-`devUrl` setup, **why the app
boots `isTauri:true` but `managed:false`** and why the workspace connect / data layer
don't come up. That is the root cause; the `fleet.sh` side (isolated `$HOME`, VNC,
dashboard, the `/tmp/tauri-mcp-<id>.sock` driving seam) is fine. Until the app boots
LOCAL (or a client-visible node-url override lands), fleet tiles should be treated as
**shared-node** — DOM/UI QA is valid, per-tile node-backed data is not isolated.
