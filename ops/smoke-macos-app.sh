#!/usr/bin/env bash
# Launch a bundled macOS Ducktape.app and assert the webview actually COMMITS
# a page. "The window exists and CEF children are running" is not enough: a
# scheme-registration mismatch between the browser process and the helper
# processes fails Mojo validation, navigations never commit, and the app
# renders a permanently blank window while every process looks healthy
# (live-diagnosed 2026-07-11). The committed-page signal is the CDP target
# list: a broken shell reports pages with an EMPTY url; a healthy one reports
# `tauri://localhost/...` plus the document title.
#
# The instance runs under a throwaway $HOME so it cannot touch the real
# workspace registry (~/.ducktape), cannot collide with a running app's
# Chromium SingletonLock, and leaves no cache behind. A window will briefly
# appear on screen — this is a local gate; the repo has no hosted CI.
set -euo pipefail

app="${1:?usage: smoke-macos-app.sh /path/to/Ducktape.app}"
binary="$app/Contents/MacOS/ducktape-desktop"
[ -x "$binary" ] || { echo "[app-smoke] missing executable $binary" >&2; exit 1; }

tmp_home="$(mktemp -d /tmp/ducktape-app-smoke.XXXXXX)"
port=""
for candidate in 9333 9433 9533 9633 9733; do
  if ! lsof -nP -iTCP:"$candidate" -sTCP:LISTEN >/dev/null 2>&1; then
    port="$candidate"
    break
  fi
done
[ -n "$port" ] || { echo "[app-smoke] no free CDP port" >&2; exit 1; }

pid=""
cleanup() {
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null || true
    for _ in $(seq 1 20); do kill -0 "$pid" 2>/dev/null || break; sleep 0.5; done
    kill -9 "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true # reap so the shell prints no job notice
  fi
  rm -rf "$tmp_home"
}
trap cleanup EXIT

HOME="$tmp_home" "$binary" --remote-debugging-port="$port" \
  >"$tmp_home/smoke-stdout.log" 2>&1 &
pid=$!

# A loaded page shows up in /json with its real URL and its parsed <title>.
# The blank-window failure mode is subtler than "no page at all": secondary
# webviews may still commit a URL with an empty title (navigation committed,
# document never arrived), so require the MAIN window's page — a
# tauri://localhost URL that is not the ?view=tray webview — to carry the
# document title, which only exists once HTML actually flowed through the
# custom-scheme handler and got parsed.
deadline=$((SECONDS + 45))
while [ $SECONDS -lt $deadline ]; do
  if ! kill -0 "$pid" 2>/dev/null; then
    echo "[app-smoke] app exited before the main page loaded:" >&2
    tail -20 "$tmp_home/smoke-stdout.log" >&2 || true
    exit 1
  fi
  targets="$(curl -fsS --max-time 2 "http://127.0.0.1:$port/json" 2>/dev/null || true)"
  if [ -n "$targets" ]; then
    # `|| true`: a torn/mid-write /json response must retry, not abort the gate.
    loaded="$(printf '%s' "$targets" | python3 -c '
import json, sys
targets = json.load(sys.stdin)
for t in targets:
    url, title = t.get("url", ""), t.get("title", "")
    if (t.get("type") == "page" and url.startswith("tauri://localhost")
        and "view=tray" not in url and title == "Ducktape"):
        print(url + " title=" + title)
        break
' 2>/dev/null || true)"
    if [ -n "$loaded" ]; then
      echo "[app-smoke] main page loaded: $loaded"
      exit 0
    fi
  fi
  sleep 1
done

echo "[app-smoke] main window never loaded tauri://localhost with a parsed title within 45s — blank-window regression?" >&2
echo "[app-smoke] CDP targets were:" >&2
printf '%s\n' "${targets:-<none>}" >&2
log="$tmp_home/Library/Caches/com.ducktape.app/cef/chrome_debug.log"
if [ -f "$log" ]; then
  echo "[app-smoke] chrome_debug.log tail (Mojo validation errors = scheme-registration mismatch):" >&2
  tail -15 "$log" >&2
fi
exit 1
