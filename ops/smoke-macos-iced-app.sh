#!/usr/bin/env bash
# Real-window smoke for the staged iced macOS bundle. It verifies the native
# main window, close-to-menu-bar semantics, and Dock/Finder activation reopen
# in an isolated HOME. System Events needs Accessibility permission for the
# terminal running this gate.
set -euo pipefail

[ "$(uname -s)" = Darwin ] \
  || { echo "[macos-smoke] macOS is required" >&2; exit 2; }

app="${1:-target/release/bundle/macos/Ducktape.app}"
binary="$app/Contents/MacOS/ducktape"
[ -x "$binary" ] \
  || { echo "[macos-smoke] missing executable $binary" >&2; exit 1; }

root="$(cd "$(dirname "$0")/.." && pwd)"
app="$(cd "$(dirname "$app")" && pwd)/$(basename "$app")"
binary="$app/Contents/MacOS/ducktape"
bash "$root/ops/check-macos-cef-bundle.sh" "$app"

tmp_home="$(mktemp -d "${TMPDIR:-/tmp}/ducktape-iced-smoke.XXXXXX")"
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

HOME="$tmp_home" "$binary" >"$tmp_home/app.log" 2>&1 &
pid=$!

window_count() {
  osascript <<APPLESCRIPT
tell application "System Events"
  set matches to every application process whose unix id is $pid
  if (count of matches) is 0 then return -1
  tell item 1 of matches to return count of windows
end tell
APPLESCRIPT
}

wait_for_windows() {
  local wanted="$1" deadline=$((SECONDS + 30)) count
  while [ "$SECONDS" -lt "$deadline" ]; do
    kill -0 "$pid" 2>/dev/null \
      || { echo "[macos-smoke] app exited early" >&2; tail -40 "$tmp_home/app.log" >&2; return 1; }
    if ! count="$(window_count 2>"$tmp_home/osascript.log")"; then
      echo "[macos-smoke] System Events could not inspect the app; grant Accessibility permission to this terminal" >&2
      cat "$tmp_home/osascript.log" >&2
      return 1
    fi
    if [ "$count" = "$wanted" ]; then return 0; fi
    sleep 0.25
  done
  echo "[macos-smoke] expected $wanted window(s), got ${count:-unknown}" >&2
  tail -40 "$tmp_home/app.log" >&2 || true
  return 1
}

wait_for_windows 1
size="$(osascript <<APPLESCRIPT
tell application "System Events"
  set appProcess to first application process whose unix id is $pid
  tell window 1 of appProcess to return size
end tell
APPLESCRIPT
)"
width="${size%%,*}"
height="${size##*, }"
if [ "$width" -lt 900 ] || [ "$height" -lt 600 ]; then
  echo "[macos-smoke] main window is below its design minimum: ${width}x${height}" >&2
  exit 1
fi
echo "[macos-smoke] native main window opened at ${width}x${height}"

osascript <<APPLESCRIPT
tell application "System Events"
  set appProcess to first application process whose unix id is $pid
  tell window 1 of appProcess
    perform action "AXPress" of first button whose subrole is "AXCloseButton"
  end tell
end tell
APPLESCRIPT
wait_for_windows 0
kill -0 "$pid"
echo "[macos-smoke] close hid the main window without quitting"

osascript <<APPLESCRIPT
tell application "System Events"
  set frontmost of first application process whose unix id is $pid to true
end tell
APPLESCRIPT
wait_for_windows 1
echo "[macos-smoke] application activation restored the main window"

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
  echo "[macos-smoke] Cmd+Q did not complete orderly shutdown" >&2
  exit 1
fi
pid=""
echo "[macos-smoke] Cmd+Q completed orderly shutdown"
echo "[macos-smoke] passed"
