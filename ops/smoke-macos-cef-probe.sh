#!/usr/bin/env bash
# Launch the bundled iced/CEF child-window probe and require both the native
# probe window and a CEF renderer helper. This catches framework/helper path,
# subprocess dispatch, architecture, signing, and native-child regressions.
set -euo pipefail

[ "$(uname -s)" = Darwin ] \
  || { echo "[macos-cef-smoke] macOS is required" >&2; exit 2; }

app="${1:-target/debug/bundle/macos/Ducktape.app}"
binary="$app/Contents/MacOS/ducktape"
[ -x "$binary" ] \
  || { echo "[macos-cef-smoke] missing executable $binary" >&2; exit 1; }
app="$(cd "$(dirname "$app")" && pwd)/$(basename "$app")"
binary="$app/Contents/MacOS/ducktape"

tmp_home="$(mktemp -d "${TMPDIR:-/tmp}/ducktape-cef-smoke.XXXXXX")"
pid=""
cleanup() {
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    kill -TERM "$pid" 2>/dev/null || true
    for _ in $(seq 1 20); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$pid" 2>/dev/null; then
      kill -KILL "$pid" 2>/dev/null || true
    fi
    wait "$pid" 2>/dev/null || true
  fi
  rm -rf "$tmp_home"
}
trap cleanup EXIT

HOME="$tmp_home" "$binary" >"$tmp_home/probe.log" 2>&1 &
pid=$!
deadline=$((SECONDS + 45))
window_ready=0
renderer_ready=0
while [ "$SECONDS" -lt "$deadline" ]; do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "[macos-cef-smoke] probe exited early" >&2
    tail -60 "$tmp_home/probe.log" >&2 || true
    exit 1
  fi
  title="$(osascript <<APPLESCRIPT 2>/dev/null || true
tell application "System Events"
  set matches to every application process whose unix id is $pid
  if (count of matches) is 0 then return ""
  tell item 1 of matches
    if (count of windows) is 0 then return ""
    return name of window 1
  end tell
end tell
APPLESCRIPT
)"
  [ "$title" = "Ducktape iced + CEF probe" ] && window_ready=1
  if ps -axo command= | grep -F "$app/Contents/Frameworks/ducktape Helper (Renderer).app/Contents/MacOS/ducktape Helper (Renderer)" | grep -q -- '--type=renderer'; then
    renderer_ready=1
  fi
  if [ "$window_ready" = 1 ] && [ "$renderer_ready" = 1 ]; then
    echo "[macos-cef-smoke] native iced window + bundled CEF renderer are live"
    osascript <<APPLESCRIPT
tell application "System Events"
  set frontmost of first application process whose unix id is $pid to true
  keystroke "q" using command down
end tell
APPLESCRIPT
    for _ in $(seq 1 40); do
      kill -0 "$pid" 2>/dev/null || break
      sleep 0.25
    done
    if kill -0 "$pid" 2>/dev/null; then
      echo "[macos-cef-smoke] Cmd+Q did not complete orderly CEF shutdown" >&2
      exit 1
    fi
    pid=""
    if ps -axo command= | grep -F "$app/Contents/Frameworks/ducktape Helper" | grep -q -- '--type='; then
      echo "[macos-cef-smoke] a bundled CEF helper survived Cmd+Q" >&2
      exit 1
    fi
    echo "[macos-cef-smoke] Cmd+Q closed the browser and all helpers"
    exit 0
  fi
  sleep 0.25
done

echo "[macos-cef-smoke] timed out (window=$window_ready renderer=$renderer_ready)" >&2
tail -60 "$tmp_home/probe.log" >&2 || true
exit 1
