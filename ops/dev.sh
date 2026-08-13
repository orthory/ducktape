#!/usr/bin/env bash
# make dev — the desktop-app dev loop against the seeded "demo" localnet.
#
# Ensures the "demo" workspace exists (first run seeds it via ops/demo-seed.sh;
# DEV_RESEED=1 forces a fresh seed), starts its node when nothing is serving
# the workspace's http endpoint, initializes its forge with ducktape's own
# repository (ops/dogfood-forge.sh), starts the three local agent services, then
# runs the desktop app in the foreground. Ctrl-C quits the app and LEAVES the
# node and services running for the next iteration — `make demo-clear` is the
# teardown.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ID="${DEMO_WORKSPACE_ID:-demo}"
WSDIR="$HOME/.ducktape/workspaces/$ID"

log(){ printf '\033[36m[dev]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[dev] %s\033[0m\n' "$*" >&2; exit 1; }

if [ ! -f "$WSDIR/node.toml" ] || [ ! -f "$WSDIR/network.toml" ] || [ -n "${DEV_RESEED:-}" ]; then
  bash "$SCRIPT_DIR/demo-seed.sh" || die "seeding the '$ID' localnet failed"
fi

# The node binary, resolved the way demo-seed resolves it.
NODE_BIN="${DUCKTAPE_NODE_BIN:-}"
if [ -z "$NODE_BIN" ]; then
  log "building ducktape (cargo build -p node-bin)…"
  blog="$(mktemp)"
  cargo build -p node-bin >"$blog" 2>&1 || die "node-bin build failed — see $blog"
  NODE_BIN="$(cargo metadata --no-deps --format-version 1 \
    | bun -e 'console.log((await Bun.stdin.json()).target_directory)')/debug/ducktape"
fi
[ -x "$NODE_BIN" ] || die "node binary not executable: $NODE_BIN"

# The workspace's app endpoint — the same `http_listen` key the app reads.
LISTEN="$(sed -n 's/^[[:space:]]*http_listen[[:space:]]*=[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
  "$WSDIR/node.toml" | head -1 | tr -d '[:space:]')"
[ -n "$LISTEN" ] || die "no http_listen in $WSDIR/node.toml"
URL="http://$LISTEN"

if curl -sf "$URL/v1/status" >/dev/null 2>&1; then
  log "node already serving at $URL"
else
  log "starting node ($URL)…"
  "$NODE_BIN" node run --config "$WSDIR/node.toml" >>"$WSDIR/dev-node.log" 2>&1 &
  NODE_PID=$!
  for _ in $(seq 1 80); do
    curl -sf "$URL/v1/status" >/dev/null 2>&1 && break
    kill -0 "$NODE_PID" 2>/dev/null || die "node exited on start — see $WSDIR/dev-node.log"
    sleep 0.25
  done
  curl -sf "$URL/v1/status" >/dev/null 2>&1 || die "node http never came up — see $WSDIR/dev-node.log"
  # The node outlives this script on purpose: the dev loop is edit → Ctrl-C →
  # `make dev` again, and rebooting a warm node every lap would cost more than
  # it protects against. `make demo-clear` sweeps it by cmdline + /v1/shutdown.
  disown "$NODE_PID"
  log "node up (pid $NODE_PID, log $WSDIR/dev-node.log)"
fi

# Shell needs all three local planes: compute owns durable runs, agent owns raw
# PTYs, and airlock lends the credential selected in the app. A warm dev loop
# keeps them beside the node; demo-clear's workspace-verified pid sweep stops
# the whole set. The service catalog is the readiness event, so do not guess
# from stale pidfiles or fixed startup time.
service_state(){
  "$NODE_BIN" service list "$1" --workspace "$WSDIR" --json 2>/dev/null |
    bun -e 'const rows = await Bun.stdin.json(); console.log(rows[0]?.state ?? "")' 2>/dev/null
}

ensure_service(){
  local kind="$1" state pid
  state="$(service_state "$kind")"
  if [ "$state" = "enabled" ]; then
    log "$kind service already running"
    return
  fi
  if [ "$state" = "signaling" ]; then
    log "$kind service is signaling but not enabled — restart it with --enable to use Shell"
    return
  fi

  log "starting $kind service…"
  "$NODE_BIN" service run "$kind" --enable --workspace "$WSDIR" >>"$WSDIR/dev-$kind.log" 2>&1 &
  pid=$!
  for _ in $(seq 1 80); do
    state="$(service_state "$kind")"
    [ "$state" = "enabled" ] && break
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.25
  done
  if [ "$state" = "enabled" ]; then
    disown "$pid"
    log "$kind service up (pid $pid, log $WSDIR/dev-$kind.log)"
  else
    if kill -0 "$pid" 2>/dev/null; then
      disown "$pid"
    else
      wait "$pid" 2>/dev/null
    fi
    log "$kind service did not become ready — Shell may be unavailable; see $WSDIR/dev-$kind.log"
  fi
}

ensure_service compute
ensure_service agent
ensure_service airlock

# The forge module starts empty, so a fresh demo node hosts no repo at all.
# Seed it with ducktape's own source (the `make dogfood-forge` script, pointed
# at THIS workspace's node) — idempotent, so later laps just refresh it.
# Non-fatal: an offline box cannot `git fetch origin`, and the app dev loop
# should not depend on that.
DUCKTAPE_DEV_FORGE_URL="$URL" bash "$SCRIPT_DIR/dogfood-forge.sh" ||
  log "forge init failed — run \`make dogfood-forge\` when origin is reachable"

log "launching the app — Ctrl-C quits the app, the node and services stay up"
exec cargo run -p ducktape-app
