# Fleet QA

Multi-instance desktop QA is owned by
[`tauri-agent-fleet`](https://github.com/byeongsu-hong/tauri-agent-fleet).
Ducktape retains only its application config, CEF build/instance hooks, and QA
suite under `tauri-agent-fleet.json` and `qa/`.

## Quick start

```bash
cd app && bun install
cd ..
app/node_modules/.bin/tauri-agent-fleet up HEAD
app/node_modules/.bin/tauri-agent-fleet status
app/node_modules/.bin/tauri-agent-fleet dashboard
app/node_modules/.bin/tauri-agent-fleet down
```

The dashboard is read-oriented and polls Fleet's live instance/run state. It
shows the revision, CEF variant, lifecycle and suite state, plugin health,
tokens/cost, artifacts, and loopback-routed noVNC screen. Lifecycle stays in the
CLI.

## How it works

```
source revision → cached CEF debug artifact → isolated instance(s) → run(s)
                                              ├─ tauri-agent direct client
                                              └─ Xvfb + loopback x11vnc
```

- `qa/fleet/build-cef.sh` builds once and copies the debug desktop plus node
  sidecar into Fleet's immutable artifact directory. Debug is deliberate: the
  tauri-agent endpoint is absent from release builds.
- `qa/fleet/prepare-instance.sh` seeds a solo Ducktape workspace using the
  cached sidecar. Fleet owns HOME, XDG/data/display/port isolation and exact
  process-group teardown.
- Ducktape's desktop is CEF-only on `dev`; the suite declares `variant: cef`.
- The dashboard and VNC server bind to loopback unless an operator explicitly
  chooses another dashboard host. Use an SSH/Tailscale tunnel for remote viewing.

## Notes

- Run the deterministic CEF smoke with
  `app/node_modules/.bin/tauri-agent-fleet test qa/suites/cef-smoke.json`.
- `status --json` is the authoritative way to find an instance ID, display,
  runtime directory, endpoint health, and artifact path.
- On hosts where x11vnc is staged rather than installed, set
  `FLEET_VNC_COMMAND` and its required `LD_LIBRARY_PATH` before invoking Fleet.
