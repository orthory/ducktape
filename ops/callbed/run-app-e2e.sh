#!/usr/bin/env bash
# App-layer huddle e2e (layer 3): run the app's REAL browser call client
# (app/src/domain/call-session.ts) in TWO headless Chromium instances with fake
# mic+camera, against the live callbed, and assert peer audio + peer video cross
# the mesh through the real capture/encode/decode path.
#
# Prereq: the callbed is up and published on the host —
#   docker compose -f ops/callbed/docker-compose.yml up -d --wait node0 node1
# (node0 -> 127.0.0.1:8080, node1 -> 127.0.0.1:8081)
#
# Builds the bundle from real app source, launches two chromium on disjoint
# debug ports + profiles, runs the CDP driver, tears chromium down.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
H="$HERE/browser-harness"
BUN="${BUN:-/home/eddy/.local/bin/bun}"
CHROME="${CHROME:-/usr/bin/chromium}"
A_PORT="${1:-8080}"; B_PORT="${2:-8081}"
TMP="$(mktemp -d)"; PA=""; PB=""
cleanup(){ [ -n "$PA" ] && kill "$PA" 2>/dev/null; [ -n "$PB" ] && kill "$PB" 2>/dev/null; wait "$PA" "$PB" 2>/dev/null; rm -rf "$TMP" 2>/dev/null; }
trap cleanup EXIT

echo "[e2e] building bundle from real app source (call-session.ts)…"
( cd "$H" && "$BUN" build entry.ts --target browser --outfile bundle.js ) \
  || { echo "[e2e] bundle build failed"; exit 2; }

launch(){ # $1 debug-port  $2 profile-name
  "$CHROME" --headless=new --no-sandbox --disable-gpu \
    --use-fake-device-for-media-stream --use-fake-ui-for-media-stream \
    --autoplay-policy=no-user-gesture-required \
    --user-data-dir="$TMP/$2" --remote-debugging-port="$1" about:blank \
    >"$TMP/$2.log" 2>&1 &
  echo $!
}
echo "[e2e] launching two headless chromium (fake mic+camera) on :9333 / :9334…"
PA=$(launch 9333 profA)
PB=$(launch 9334 profB)

echo "[e2e] driving the real call client over CDP…"
( cd "$H" && "$BUN" drive.ts "$A_PORT" "$B_PORT" general 9333 9334 )
RC=$?
echo "[e2e] driver exit=$RC"
exit $RC
