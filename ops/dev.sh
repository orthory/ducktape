#!/usr/bin/env bash
# make dev — the desktop app with a HOT-RELOADING node.
#
# Runs `tauri dev` (frontend hot-reload) AND watches the Rust tree; when any
# node/kernel source changes it rebuilds ducktape-node and restarts the running
# node IN PLACE (same --config). The app resolves the node via DUCKTAPE_NODE_BIN
# and adopts the fresh process on its next liveness heartbeat, so a Rust change
# shows up without you touching the app.
#
# A node-logic change in dev is non-consensus-breaking, so a plain restart
# (re-sync from durable storage) is the correct rollover — no governance upgrade.
#
# Assumes no other `tauri dev` is already running (it owns vite :1430); stop any
# existing app first.
set -uo pipefail
cd "$(dirname "$0")/.."
ROOT="$PWD"

CARGO="${CARGO:-cargo}"
BUN="${BUN:-bun}"
NODE_BIN="$ROOT/target/debug/ducktape-node"
# The app checks DUCKTAPE_NODE_BIN first (app/src-tauri/src/daemon.rs), so pin it
# to our debug build — the exact binary this script rebuilds and restarts.
export DUCKTAPE_NODE_BIN="$NODE_BIN"
# Keep the idle dev chain quiet: no nop heartbeat blocks, so the telemetry panel
# shows every block (all real activity) instead of a heartbeat stream, and an
# idle node honestly reads as empty. dev is single-validator with no coordinated
# upgrades, so the heartbeat earns nothing here (see bin/node/src/main.rs).
export DUCKTAPE_DISABLE_HEARTBEAT=1

log() { printf '\033[36m[dev]\033[0m %s\n' "$*"; }

spawn_node() { # $1 = config path; detached orphan, mirrors the app's own spawn
  nohup "$NODE_BIN" --config "$1" >>"${1%/*}/dev-node.log" 2>&1 &
  disown 2>/dev/null || true
}

restart_node() {
  log "rust changed → rebuilding ducktape-node…"
  if ! $CARGO build -p node-bin; then
    log "✗ build failed — leaving the running node up"
    return
  fi
  local pid cfg i=0
  pid=$(pgrep -f "$NODE_BIN --config" | head -1 || true)
  if [ -z "${pid:-}" ]; then
    log "✓ built; no live node — the app will spawn the fresh binary itself"
    return
  fi
  cfg=$(ps -o command= -p "$pid" | sed -n 's/.*--config \([^ ][^ ]*\).*/\1/p')
  [ -n "$cfg" ] || { log "could not read node --config; skipping restart"; return; }
  log "restarting node (pid $pid) on $cfg…"
  kill "$pid" 2>/dev/null || true
  while kill -0 "$pid" 2>/dev/null && [ $i -lt 60 ]; do sleep 0.1; i=$((i + 1)); done
  spawn_node "$cfg"
  log "✓ node back on the fresh binary (app reconnects on its next heartbeat)"
}

watch_rust() { # zero-dep poll (no cargo-watch/watchexec on this box)
  local stamp
  stamp="$(mktemp)"
  while :; do
    if [ -n "$(find bin crates -name '*.rs' -newer "$stamp" -print -quit 2>/dev/null)" ]; then
      touch "$stamp"
      restart_node
    fi
    sleep 2
  done
}

log "building ducktape-node (debug)…"
$CARGO build -p node-bin || {
  log "initial node build failed"
  exit 1
}

# Stop any node left over from a previous session: the app ADOPTS an
# already-running node by port (app/src-tauri/src/workspaces.rs), so a stale one
# would be picked up instead of our fresh binary (no telemetry, still
# heartbeating). Killing it makes the app spawn a fresh node — which inherits
# DUCKTAPE_NODE_BIN + DUCKTAPE_DISABLE_HEARTBEAT from this shell.
if pkill -f 'ducktape-node --config' 2>/dev/null; then
  log "stopped a stale node from a previous session"
  sleep 0.5
fi

# Skip the slow release-sidecar step in beforeDevCommand: in dev the app uses
# DUCKTAPE_NODE_BIN, and build.rs leaves a placeholder that satisfies tauri's
# externalBin. This makes startup fast; the sidecar is only for `make app`.
CFG_OVERRIDE="${TMPDIR:-/tmp}/ducktape-dev-tauri-$$.json"
printf '{"build":{"beforeDevCommand":"%s run dev"}}\n' "$BUN" >"$CFG_OVERRIDE"

watch_rust &
WATCH_PID=$!
cleanup() {
  kill "$WATCH_PID" 2>/dev/null || true
  rm -f "$CFG_OVERRIDE"
}
trap cleanup EXIT INT TERM

log "launching tauri dev (frontend hot-reload; Ctrl-C to stop)…"
cd app
"$BUN" run tauri dev --config "$CFG_OVERRIDE"
