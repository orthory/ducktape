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
# The dev node is pinned OUTSIDE target/ (see main): `tauri dev` stages the
# externalBin placeholder onto target/debug/ducktape-node, which would clobber
# the real node cargo builds there. A staged copy the app dials keeps the two
# from fighting.
#
# Assumes no other `tauri dev` is already running (it owns vite :1430); this is
# preflight-checked, and the stale-node sweep is scoped to THIS worktree, so a
# sibling worktree's or a fleet tile's node is never touched.
set -uo pipefail

CARGO="${CARGO:-cargo}"
BUN="${BUN:-bun}"
CEF_CLONE="${CEF_CLONE:-$HOME/.cache/ducktape-cef-probe/tauri-cef}"

log() { printf '\033[36m[dev]\033[0m %s\n' "$*"; }

dev_os() {
  printf '%s\n' "${DUCKTAPE_DEV_OS:-$(uname -s)}"
}

app_deps_need_install() { # $1 = app dir
  local app_dir="$1"
  [ -d "$app_dir/node_modules" ] || return 0
  [ "$app_dir/package.json" -nt "$app_dir/node_modules" ] && return 0
  [ "$app_dir/bun.lock" -nt "$app_dir/node_modules" ] && return 0
  [ -d "$app_dir/node_modules/@byeongsu-hong/tauri-agent-plugin" ] || return 0
  return 1
}

ensure_app_deps() { # $1 = app dir
  local app_dir="$1"
  if app_deps_need_install "$app_dir"; then
    log "installing app dependencies…"
    (cd "$app_dir" && "$BUN" install --frozen-lockfile) || return 1
    touch "$app_dir/node_modules"
  fi
}

# 0 iff 127.0.0.1:$1 accepts a connection right now (mirrors the app's
# port_listening liveness probe; bash /dev/tcp, no nc dependency).
port_probe() { # $1 = port
  (exec 3<>"/dev/tcp/127.0.0.1/$1") 2>/dev/null
}

# PIDs of THIS worktree's node — matched against our absolute $NODE_BIN as a
# FIXED string, never a regex: a worktree path can carry regex metachars
# (+, ., []), so
# `pgrep -f`'s regex would mis- or over-match. Linux reads /proc argv
# (NUL-delimited → exact); the fallback is ps + awk index() (fixed-string).
node_pids() {
  if [ -n "${DUCKTAPE_DEV_NODE_PIDS:-}" ]; then
    printf '%s\n' $DUCKTAPE_DEV_NODE_PIDS
    return
  fi
  if [ -d /proc ]; then
    local p cmd
    for p in /proc/[0-9]*; do
      [ -r "$p/cmdline" ] || continue
      cmd=$(tr '\0' ' ' <"$p/cmdline" 2>/dev/null)
      case "$cmd" in
        *"$NODE_BIN --config"*) printf '%s\n' "${p#/proc/}" ;;
      esac
    done
  else
    ps -eo pid=,args= 2>/dev/null \
      | awk -v m="$NODE_BIN --config" 'index($0, m) && $0 !~ /awk/ { print $1 }'
  fi
}

# The value passed to --config in a pid's argv (handles spaces in the path).
node_config_of() { # $1 = pid
  if [ -n "${DUCKTAPE_DEV_NODE_CONFIG:-}" ]; then
    printf '%s\n' "$DUCKTAPE_DEV_NODE_CONFIG"
    return
  fi
  if [ -r "/proc/$1/cmdline" ]; then
    tr '\0' '\n' <"/proc/$1/cmdline" | awk 'p { print; exit } /^--config$/ { p = 1 }'
  else
    ps -o command= -p "$1" 2>/dev/null | sed -n 's/.*--config \([^ ][^ ]*\).*/\1/p'
  fi
}

# Copy the freshly-built node to the pinned out-of-target path. rm-first: a
# running node holds the old inode, so replacing the file never ETXTBSYs and the
# live process keeps executing until we bounce it; a fresh inode also can't
# inherit a stale mode.
stage_node() {
  mkdir -p "${NODE_BIN%/*}" || return 1
  rm -f "$NODE_BIN"
  cp "$NODE_SRC" "$NODE_BIN"
}

spawn_node() { # $1 = config path; detached orphan, mirrors the app's own spawn.
  local dir="${1%/*}"
  # Write to the SAME daemon.log the app reads (workspaces.rs classify /
  # workspace_log_tail), so a rebuilt node's panic/bind-error is visible to the
  # app instead of stranded in a side file. Update node.pid so the app's
  # teardown AND its process-death phase check track the LIVE node, not the one
  # it first spawned (else a hot-reload would read as a fatal crash).
  nohup "$NODE_BIN" --config "$1" >>"$dir/daemon.log" 2>&1 &
  SPAWNED_PID=$!
  printf '%s\n' "$SPAWNED_PID" >"$dir/node.pid" 2>/dev/null || true
}

restart_node() {
  log "rust changed → rebuilding ducktape-node…"
  local before after
  before=$(cksum "$NODE_SRC" 2>/dev/null | cut -d' ' -f1-2)
  if ! $CARGO build -p node-bin; then
    log "✗ build failed — leaving the running node up"
    return
  fi
  after=$(cksum "$NODE_SRC" 2>/dev/null | cut -d' ' -f1-2)
  # Hash-gate: a test/comment/doc edit rebuilds but produces the same binary —
  # don't bounce the node (dropping ws/huddle state) for a no-op change.
  if [ -n "$before" ] && [ "$before" = "$after" ]; then
    log "· node binary unchanged — skipping restart"
    return
  fi

  local pid cfg dir i=0
  pid=$(node_pids | head -1)
  if ! stage_node; then
    log "✗ could not stage the fresh node to $NODE_BIN — leaving the running node up"
    return
  fi
  if [ -z "${pid:-}" ]; then
    log "✓ built + staged; no live node — the app will spawn the fresh binary itself"
    return
  fi
  cfg=$(node_config_of "$pid")
  [ -n "$cfg" ] || {
    log "could not read node --config; skipping restart"
    return
  }
  dir="${cfg%/*}"
  log "restarting node (pid $pid) on ${cfg}…"
  kill "$pid" 2>/dev/null || true
  while kill -0 "$pid" 2>/dev/null && [ $i -lt 60 ]; do
    sleep 0.1
    i=$((i + 1))
  done
  # Escalate: a node that ignored SIGTERM still holds the ports, so the fresh one
  # would die on EADDRINUSE under a false "✓". SIGKILL it and wait for it to go.
  if kill -0 "$pid" 2>/dev/null; then
    log "old node ignored SIGTERM after 6s — sending SIGKILL"
    kill -9 "$pid" 2>/dev/null || true
    i=0
    while kill -0 "$pid" 2>/dev/null && [ $i -lt 30 ]; do
      sleep 0.1
      i=$((i + 1))
    done
  fi

  spawn_node "$cfg"
  # VERIFY — never claim success over a corpse. Give it a moment, then confirm
  # the process is alive; on death, tail the log the node just wrote so the real
  # reason (bind conflict, bad config, panic) is right there in the terminal.
  sleep 0.4
  if ! jobs -pr | grep -qx "${SPAWNED_PID:-0}"; then
    wait "${SPAWNED_PID:-0}" 2>/dev/null || true
    log "✗ rebuilt node exited on start — last log lines:"
    tail -n 20 "$dir/daemon.log" 2>/dev/null | sed 's/^/    /'
    return
  fi
  disown "$SPAWNED_PID" 2>/dev/null || true
  log "✓ node back (pid $SPAWNED_PID) on the fresh binary; app reconnects on its next heartbeat"
}

watch_rust() { # zero-dep poll (no cargo-watch/watchexec on this box)
  local stamp
  stamp="$(mktemp)"
  STAMP_FILE="$stamp"
  while :; do
    if [ -n "$(find bin crates -name '*.rs' -newer "$stamp" -print -quit 2>/dev/null)" ]; then
      touch "$stamp"
      restart_node
    fi
    sleep 2
  done
}

cleanup() {
  kill "${WATCH_PID:-}" 2>/dev/null || true
  rm -f "${CFG_OVERRIDE:-}" "${STAMP_FILE:-}"
}

macos_helper_names() {
  printf '%s\n' \
    "ducktape-desktop Helper" \
    "ducktape-desktop Helper (Alerts)" \
    "ducktape-desktop Helper (GPU)" \
    "ducktape-desktop Helper (Plugin)" \
    "ducktape-desktop Helper (Renderer)"
}

macos_debug_app_path() {
  printf '%s\n' "${MACOS_DEBUG_APP:-$ROOT/target/debug/Ducktape.app}"
}

macos_bundle_source() {
  local candidate
  for candidate in \
    "${MACOS_BUNDLE_SOURCE:-}" \
    "$ROOT/target/debug/bundle/macos/Ducktape.app" \
    "$ROOT/target/release/bundle/macos/Ducktape.app"
  do
    [ -n "$candidate" ] || continue
    if [ -d "$candidate" ]; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  return 1
}

ensure_macos_bundle_skeleton() {
  if macos_bundle_source >/dev/null; then
    return 0
  fi
  [ -d "$CEF_CLONE/crates/tauri-cli" ] || {
    log "✗ missing CEF Tauri CLI checkout at $CEF_CLONE"
    return 1
  }
  log "building macOS CEF bundle skeleton…"
  (
    cd "$ROOT/app" || exit 1
    "$BUILD_WITH" "$CARGO" run \
      --manifest-path "$CEF_CLONE/crates/tauri-cli/Cargo.toml" \
      --bin cargo-tauri -- build --debug --bundles app --ignore-version-mismatches \
      --config '{"build":{"beforeBuildCommand":"bun run build && ../ops/stage-debug-sidecar.sh"}}'
  )
}

stage_macos_debug_bundle() { # $1 = target/debug/ducktape-desktop
  local binary="${1:?stage_macos_debug_bundle needs a debug executable}"
  local app source contents helper helper_exe
  [ -x "$binary" ] || {
    log "✗ missing debug executable $binary"
    return 1
  }
  source=$(macos_bundle_source) || {
    log "✗ no Ducktape.app bundle skeleton found"
    return 1
  }
  app=$(macos_debug_app_path)
  rm -rf "$app"
  mkdir -p "${app%/*}"
  cp -R "$source" "$app"
  contents="$app/Contents"
  install -m 755 "$binary" "$contents/MacOS/ducktape-desktop"
  while IFS= read -r helper; do
    helper_exe="$contents/Frameworks/$helper.app/Contents/MacOS/$helper"
    [ -d "${helper_exe%/*}" ] || {
      log "✗ bundle skeleton missing $helper.app"
      return 1
    }
    install -m 755 "$binary" "$helper_exe"
  done < <(macos_helper_names)
  bash "$ROOT/ops/check-macos-cef-bundle.sh" "$app" >/dev/null
  printf '%s\n' "$app"
}

run_tauri_dev() {
  case "$(dev_os)" in
    Darwin)
      ensure_macos_bundle_skeleton || exit 1
      log "launching tauri dev through bundled macOS runner (frontend hot-reload; Ctrl-C to stop)…"
      "$BUILD_WITH" "$CARGO" run \
        --manifest-path "$CEF_CLONE/crates/tauri-cli/Cargo.toml" \
        --bin cargo-tauri -- dev --config "$CFG_OVERRIDE" --runner "$ROOT/ops/dev-macos-runner.sh"
      ;;
    *)
      log "launching tauri dev (frontend hot-reload; Ctrl-C to stop)…"
      "$BUN" run tauri dev --config "$CFG_OVERRIDE"
      ;;
  esac
}

main() {
  cd "$(dirname "$0")/.." || {
    echo "[dev] ✗ cannot cd to the repo root from $0" >&2
    exit 1
  }
  ROOT="$PWD"
  BUILD_WITH="${BUILD_WITH:-$ROOT/ops/build-with.sh}"
  NODE_SRC="$ROOT/target/debug/ducktape-node"
  # Pin the dev node OUTSIDE target/ (see the file header): a stable per-worktree
  # path so tauri's externalBin placeholder copy can't truncate the node the app
  # dials. The app checks DUCKTAPE_NODE_BIN first (app/src-tauri/src/daemon.rs).
  local tag
  tag=$(printf '%s' "$ROOT" | cksum | cut -d' ' -f1)
  NODE_BIN="${TMPDIR:-/tmp}/ducktape-dev-node-$(id -u)-$tag/ducktape-node"
  export DUCKTAPE_NODE_BIN="$NODE_BIN"
  # Keep the idle dev chain quiet: no nop heartbeat blocks (dev is single-
  # validator with no coordinated upgrades, so the heartbeat earns nothing).
  export DUCKTAPE_DISABLE_HEARTBEAT=1

  ensure_app_deps "$ROOT/app" || {
    log "app dependency install failed"
    exit 1
  }

  log "building ducktape-node (debug)…"
  $CARGO build -p node-bin || {
    log "initial node build failed"
    exit 1
  }
  stage_node || {
    log "could not stage the dev node to $NODE_BIN"
    exit 1
  }

  # Preflight BEFORE the destructive sweep: if :1430 is already owned (another
  # tauri dev / vite), abort naming nothing-killed, rather than tearing down our
  # own node just to collide on the port.
  if port_probe 1430; then
    log "✗ :1430 is already in use — another 'tauri dev'? Stop it first. Nothing was killed."
    exit 1
  fi

  # Reap ONLY this worktree's leftover node (scoped to our absolute $NODE_BIN).
  # The app adopts a listening node by port, so a stale one would be picked up
  # instead of our fresh build.
  local stale
  stale=$(node_pids)
  if [ -n "$stale" ]; then
    # shellcheck disable=SC2086
    kill $stale 2>/dev/null || true
    log "stopped this worktree's stale node(s): $(printf '%s ' $stale)"
    sleep 0.5
  fi

  # Skip the slow release-sidecar step in beforeDevCommand: in dev the app uses
  # DUCKTAPE_NODE_BIN and build.rs leaves a placeholder that satisfies tauri's
  # externalBin. The sidecar is only for `make app`.
  CFG_OVERRIDE="${TMPDIR:-/tmp}/ducktape-dev-tauri-$$.json"
  if ! printf '{"build":{"beforeDevCommand":"%s run dev"}}\n' "$BUN" >"$CFG_OVERRIDE"; then
    log "✗ could not write the dev tauri config to $CFG_OVERRIDE (check TMPDIR/disk)"
    exit 1
  fi

  trap cleanup EXIT INT TERM
  if command -v find >/dev/null 2>&1; then
    watch_rust &
    WATCH_PID=$!
  else
    log "✗ 'find' not on PATH — Rust hot-reload disabled (rebuild + restart the app manually)"
  fi

  cd app || {
    log "✗ app/ not found from $ROOT"
    exit 1
  }
  run_tauri_dev
}

# Sourced by ops/dev.test.sh for unit tests? define the functions, run nothing.
[ "${DEV_SH_LIB:-}" = 1 ] && return 0
main "$@"
