# Fleet QA

Multi-instance desktop QA is owned by
[`tauri-agent-fleet`](https://github.com/byeongsu-hong/tauri-agent-fleet).
Ducktape retains only its application config and suites under `.tauri-agent/`
plus its CEF build/instance hooks under `qa/fleet/`.

## Quick start

```bash
cd app && bun install
cd ..
FLEET="${FLEET:-app/node_modules/@byeongsu-hong/tauri-agent-fleet/dist/cli.js}"
"$FLEET" up HEAD
"$FLEET" status
"$FLEET" dashboard
"$FLEET" down
```

During Fleet development, point `FLEET` at a locally built `dist/cli.js`.
Update Ducktape's dependency pin only after that Fleet revision is reviewed.

The dashboard is read-oriented and polls Fleet's live instance/run state. It
shows the revision, CEF runtime, lifecycle and suite state, agent health,
tokens/cost, artifacts, and the available observation capability. Linux exposes
a loopback-routed noVNC screen. macOS launches the native window and reports VNC
as unavailable instead of advertising a fake route. Lifecycle stays in the CLI.

## How it works

```
source revision → cached CEF debug artifact → isolated instance(s) → run(s)
                                              ├─ tauri-agent direct client
                                              ├─ Linux: Xvfb + loopback x11vnc
                                              └─ macOS: native app window
```

- `qa/fleet/build-cef.sh` builds once and stages the debug desktop plus node
  sidecar in Fleet's immutable artifact directory. macOS receives a complete
  CEF `.app` bundle; Linux retains its flat executable and CEF resources. Debug
  is deliberate: the tauri-agent endpoint is absent from release builds.
- `qa/fleet/prepare-instance.sh` seeds a solo Ducktape workspace using the
  cached sidecar. `qa/fleet/cleanup-instance.ts` verifies the workspace pidfile,
  executable, config path, and start identity before stopping the detached node
  group. Fleet owns HOME, XDG/data/display/port isolation and its desktop
  process group. Linux additionally owns the VNC and X process groups; macOS
  uses `display=native` and `vncPort=0`.
- **Stop the instance before deleting its worktree.** `cleanupInstance` is a
  path *inside* the worktree, while the workspace, pidfile, and detached node
  live outside it under `FLEET_HOME` — so removing the worktree first destroys
  the only thing that could stop the node, and it survives forever, unreachable
  by `fleet down`. `ops/worktree-clean.sh` does the sequence in the right order
  (dry-run by default, `--yes` to act): it reaps orphaned instances, then
  removes worktrees whose branch is fully merged into `origin/dev`, and refuses
  any that is dirty or carries an unmerged commit.
- Ducktape's desktop is CEF-only on `dev`; the suite declares `runtime: cef`.
- The dashboard and Linux VNC server bind to loopback unless an operator
  explicitly chooses another dashboard host. Use an SSH/Tailscale tunnel for
  remote Linux viewing. macOS has no Fleet VNC route.

## Notes

- Run the deterministic CEF smoke with
  `app/node_modules/@byeongsu-hong/tauri-agent-fleet/dist/cli.js test cef-smoke`.
- On an Apple Silicon Mac, run `ops/smoke-macos-sandbox.sh` before changing
  sandbox config, provider lifecycle, or managed-node restart code. It gates a
  real Podman run, a real ephemeral Tart run, shutdown broadcast behavior, UI
  Apply for both modes, and failed-boot rollback. The hook's fixed QA password
  is opt-in and exists only inside Fleet's throwaway HOME.
- `status --json` is the authoritative way to find an instance ID, display,
  runtime directory, agent health, and artifact path.
- On Linux hosts where x11vnc is staged rather than installed, set
  `FLEET_VNC_COMMAND` and its required `LD_LIBRARY_PATH` before invoking Fleet.
