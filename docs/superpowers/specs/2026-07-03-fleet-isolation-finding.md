# Fleet isolation finding — SOLVED (root cause + fix)

- Date: 2026-07-03
- Context: reconciling agent-driven QA into the fleet dashboard (PR #82 / #85).
- **Status: root cause found and fixed. See PR #90 (merged to `dev`).**

## Symptom

Fleet tiles were not isolated: every app showed `isTauri:true` but
`state.managed===false` (REMOTE badge), no per-workspace data — they all looked
like the shared `8844`, so the isolated `$HOME` seemed useless.

## Root cause (NOT env/plumbing — a React StrictMode boot race)

`DucktapeProvider`'s boot effect resolves the active `~/.ducktape` workspace
asynchronously, guarded by a `bootStartedRef` so it runs once. Under React
StrictMode (dev) the effect mounts → unmounts → remounts:

- mount 1 sets the guard true and starts the async resolve;
- cleanup sets `cancelled = true`;
- mount 2 sees the guard already true → **early-returns** and never restarts;
- mount 1's async then resolves, sees `cancelled` → **bails**.

So `connectActive` never fires → the app never connects its workspace node →
stuck unmanaged, even with a valid active workspace and a live node. It only
surfaces when the `workspace_active`/`workspace_select` invokes are slow enough
that cleanup wins the race — i.e. headless `tauri dev`.

**This is why every prior "fix" failed** (they were all downstream of a boot that
never connected): `VITE_DUCKTAPE_NODE_URL` via env / `vite.config` `define` /
`.env.local`, and deferring the mount for `__TAURI_INTERNALS__`. All red herrings.
`isTauri()` was true and the desktop branch *was* taken — it just got dropped.

## The fix (PR #90, merged to `dev`)

Reset the guard in the effect cleanup so the StrictMode remount re-runs the boot:

```ts
return () => {
  cancelled = true;
  bootStartedRef.current = false;   // <- the fix
};
```

`connectActive` is idempotent (`workspace_select` adopts an already-listening node
rather than double-spawning), so re-running the boot is safe.

**Verified end-to-end** on the headless app: with #90 + a valid active workspace +
its node up, it boots **LOCAL** and renders that node's data — a channel created on
the workspace node appears in the app (distinct from shared 8844). Previously stuck
REMOTE with no data. app vitest green.

## fleet.sh follow-on (to make tiles isolated)

With #90 in, each fleet worktree's app boots StrictMode-safe. To make a tile show a
**live isolated** workspace (not the onboarding gate), `fleet.sh up_one` should, per
instance:

1. **Rebase the worktree onto `dev`** so its app carries #90.
2. Stage a stable `ducktape-node` **outside the shared target dir** (the tauri build
   overwrites `target/debug/ducktape-node` with an empty placeholder → spawning it
   fails `permission denied`); pass it as **`DUCKTAPE_NODE_BIN`** on the app env.
3. **Seed a solo workspace** into the isolated `$HOME/.ducktape`: run the same verbs
   `workspace_create` uses — `ducktape-node init --name <id> --dir <dir>
   --listen/--advertised/--http/--rpc …` then `keygen --out <dir>/identity.key` — and
   write `registry.json` with that workspace `active`.

Then the app boots straight to LOCAL on its own node (`workspace_select` spawns/adopts
it via `DUCKTAPE_NODE_BIN`). No `VITE_DUCKTAPE_NODE_URL` needed — LOCAL mode uses the
workspace, not that env. `fleet.sh`'s existing side (isolated `$HOME`, VNC, dashboard,
the `/tmp/tauri-mcp-<id>.sock` seam) is unchanged.
