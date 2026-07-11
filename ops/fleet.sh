#!/usr/bin/env bash
# fleet.sh — run the real Ducktape desktop app for EACH git worktree, headless,
# and expose them all through ONE token-multiplexed web port so the fleet-console
# can show a live grid of the apps your agents are driving/QAing.
#
#   ops/fleet.sh up [branch…]     bring up an instance per worktree (all, or named)
#   ops/fleet.sh down [branch…]   tear down all, or the named instances
#   ops/fleet.sh status           per-worktree slot/ports/status + connect URL
#   ops/fleet.sh refresh          re-emit fleet.json (no restart)
#   ops/fleet.sh build-console    bun install + vite build the console into dist/
#   ops/fleet.sh url              print the dashboard URL
#
# Userspace only (no root). Reuses the x11vnc/xdotool tools staged by remote-tauri
# under ~/.local/opt/remote-tauri. Port bases are offset from the
# single-instance remote-tauri (:99/5900/6080) so both can run at once.
#
# Per-instance isolation: each app runs with its own HOME so ~/.ducktape (the
# workspace registry) and the app-data dir don't collide; CARGO_HOME/RUSTUP_HOME/
# caches are pinned to the REAL home so builds stay warm. Node ports auto-allocate
# via the app's own bind(:0), so daemons never collide.
set -uo pipefail

# ── paths & staged tools ────────────────────────────────
SELF="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CONSOLE_DIR="$SELF/fleet-console"
DIST="$CONSOLE_DIR/dist"
REAL_HOME="$HOME"
PREFIX="$REAL_HOME/.local/opt/remote-tauri"
STATE="$PREFIX/fleet"
TOKENS="$STATE/tokens"
NODE_BIN="$PREFIX/bin/ducktape-node"   # stable node bin, staged OUTSIDE any target/
X11VNC="$PREFIX/root/usr/bin/x11vnc"
XDO="$PREFIX/root/usr/bin/xdotool"
export LD_LIBRARY_PATH="$PREFIX/root/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"

COMMON_DIR="$(git -C "$SELF" rev-parse --path-format=absolute --git-common-dir)"
MAIN_ROOT="$(dirname "$COMMON_DIR")"

# ── config (env-overridable) ────────────────────────────
BASE_BRANCH="${FLEET_BASE:-dev}"
DISP_BASE="${FLEET_DISP_BASE:-110}"     # DISPLAY :110, :111, …
VITE_BASE="${FLEET_VITE_BASE:-1430}"    # vite 1430, 1431, …
VNC_BASE="${FLEET_VNC_BASE:-5910}"      # x11vnc 5910, 5911, … (localhost only)
WEB_PORT="${FLEET_WEB_PORT:-6090}"      # the ONE exposed port
SCREEN="${FLEET_SCREEN:-1400x900x24}"
TSIP="$(tailscale ip -4 2>/dev/null | head -1)"; TSIP="${TSIP:-127.0.0.1}"

mkdir -p "$STATE" "$TOKENS"

log(){ printf '  %s\n' "$*"; }
port_up(){ ss -ltn 2>/dev/null | grep -q "127.0.0.1:$1 \|$TSIP:$1 \|0.0.0.0:$1 \|\*:$1 "; }
slug(){ echo "$1" | tr '[:upper:]' '[:lower:]' | sed 's#[^a-z0-9]#-#g; s#--*#-#g; s#^-##; s#-$##'; }

# ── worktree discovery ──────────────────────────────────
# emits TSV rows: <path>\t<branch>\t<id>   (one per worktree with an app/ dir)
worktrees(){
  git -C "$MAIN_ROOT" worktree list --porcelain | awk '
    /^worktree /   { path=$2 }
    /^branch /     { br=$2; sub("refs/heads/","",br);
                     print path "\t" br }
    /^detached$/   { print path "\tDETACHED" }
  ' | while IFS=$'\t' read -r path br; do
        [ -d "$path/app" ] || continue
        printf '%s\t%s\t%s\n' "$path" "$br" "$(slug "$br")"
      done
}

# ── persistent slot assignment (id → slot, lowest free, reused) ──
slot_for(){ bun "$SELF/fleet.mjs" slot "$STATE/slots.json" "$1"; }

# ── isolation: stable node bin + a seeded solo workspace per instance ───
# Without these the app boots REMOTE and every tile shares 127.0.0.1:8844. The
# app must also carry PR #90 (StrictMode boot fix, on dev) to actually reach
# LOCAL — worktrees behind dev boot REMOTE regardless.
ensure_node_bin(){
  [ -x "$NODE_BIN" ] && [ -s "$NODE_BIN" ] && return 0
  mkdir -p "$(dirname "$NODE_BIN")"
  log "staging ducktape-node (cargo build -p node-bin)…"
  ( cd "$MAIN_ROOT" && cargo build -p node-bin >"$STATE/node-build.log" 2>&1 ) \
    || { log "node-bin build FAILED — see $STATE/node-build.log"; return 1; }
  # copy OUT of target/: tauri dev's build.rs overwrites target/debug/ducktape-node
  # with a 0-byte placeholder, which spawns permission-denied.
  cp "$MAIN_ROOT/target/debug/ducktape-node" "$NODE_BIN"
  log "staged $NODE_BIN"
}

# Seed the isolated HOME's ~/.ducktape with a solo workspace, marked active, so
# the app boots straight to LOCAL on its OWN node. registry.json is camelCase
# (the Workspace/Registry structs are #[serde(rename_all="camelCase")]).
seed_workspace(){
  local id="$1" home="$2"
  local reg="$home/.ducktape/registry.json" dir="$home/.ducktape/workspaces/$1"
  [ -f "$reg" ] && return 0
  mkdir -p "$dir"
  local p1 p2 p3
  read -r p1 p2 p3 < <(bun "$SELF/fleet.mjs" ports 3)
  local chain pub
  chain="$("$NODE_BIN" init --name "$id" --dir "$dir" \
    --listen 127.0.0.1:$p1 --advertised 127.0.0.1:$p1 --http 127.0.0.1:$p2 --rpc 127.0.0.1:$p3 \
    2>/dev/null | tail -1)"
  pub="$("$NODE_BIN" keygen --out "$dir/identity.key" 2>/dev/null | tail -1)"
  cat >"$reg" <<JSON
{"version":1,"active":"$id","workspaces":[{"id":"$id","name":"$id","chainId":"$chain","pubkey":"$pub","founder":true,"member":true,"ports":{"listen":$p1,"http":$p2,"rpc":$p3}}]}
JSON
  log "[$id] seeded solo workspace (own node http 127.0.0.1:$p2)"
}

# ── bring up ONE worktree instance ──────────────────────
up_one(){
  local path="$1" branch="$2" id="$3"
  local slot; slot="$(slot_for "$id")"
  local disp=":$((DISP_BASE+slot))" vite=$((VITE_BASE+slot)) vnc=$((VNC_BASE+slot))
  local home="$STATE/$id/home" wsdir="$STATE/$id"
  local endpoint="$wsdir/tauri-agent/com.ducktape.app/endpoint.json"
  local app="$path/app"
  mkdir -p "$home" "$wsdir"
  chmod 700 "$wsdir"   # XDG_RUNTIME_DIR for this instance — must be user-private
  log "[$id] slot $slot  disp $disp  vite $vite  vnc $vnc"

  # real isolation: stage the node bin once + seed this instance's own workspace
  ensure_node_bin && seed_workspace "$id" "$home"

  # Xvfb (setsid so it outlives the invoking shell / tool session)
  pgrep -f "Xvfb $disp " >/dev/null 2>&1 || \
    setsid Xvfb "$disp" -screen 0 "$SCREEN" -nolisten tcp >"$wsdir/xvfb.log" 2>&1 < /dev/null &
  sleep 1

  # frontend deps (bun cache pinned to real home so this stays warm)
  [ -d "$app/node_modules" ] || ( cd "$app" && \
    BUN_INSTALL_CACHE_DIR="$REAL_HOME/.bun/install/cache" bun install >"$wsdir/bun-install.log" 2>&1 )

  # vite dev server (strict port)
  port_up "$vite" || ( cd "$app" && DUCKTAPE_TAURI_DEV_PORT="$vite" \
    setsid bun run dev >"$wsdir/vite.log" 2>&1 < /dev/null & )

  # tauri config override: skip the slow beforeDevCommand, point at our vite
  cat >"$wsdir/no-before.json" <<JSON
{ "build": { "beforeDevCommand": null, "devUrl": "http://localhost:$vite" } }
JSON

  # a dead instance leaves a stale endpoint file that would block restart; if the
  # VNC (started after the app) is gone, the instance is dead — clear it.
  if [ -f "$endpoint" ] && ! port_up "$vnc"; then rm -f "$endpoint"; fi

  # the app — isolated HOME, warm build caches, headless WebKit flags
  if ! [ -f "$endpoint" ]; then
    ( cd "$app" && \
      HOME="$home" \
      CEF_PATH="${CEF_PATH:-$REAL_HOME/.local/share/cef}" \
      CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      XDG_CACHE_HOME="$REAL_HOME/.cache" BUN_INSTALL_CACHE_DIR="$REAL_HOME/.bun/install/cache" \
      PATH="$REAL_HOME/.local/bin:$PATH" \
      DISPLAY="$disp" WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
      LIBGL_ALWAYS_SOFTWARE=1 GDK_BACKEND=x11 \
      DUCKTAPE_TAURI_DEV_PORT="$vite" XDG_RUNTIME_DIR="$wsdir" \
      DUCKTAPE_NODE_BIN="$NODE_BIN" \
      setsid dbus-run-session -- bunx tauri dev --config "$wsdir/no-before.json" --no-dev-server-wait \
      >"$wsdir/tauri.log" 2>&1 < /dev/null & )
    log "[$id] app starting (compiling if cold)…"
  fi

  # x11vnc on localhost only — the browser reaches it via the token router
  port_up "$vnc" || "$X11VNC" -display "$disp" -nopw -listen 127.0.0.1 -rfbport "$vnc" \
    -forever -shared -noxdamage -ncache 0 -bg -o "$wsdir/x11vnc.log" >/dev/null 2>&1

  # token → this instance's VNC (TokenFile: one file per token)
  echo "$id: 127.0.0.1:$vnc" >"$TOKENS/$id"
}

# focus + fill the app window on a display (keyboard + no blank margins)
focus_fill(){
  local disp="$1" fbw fbh wid
  fbw="${SCREEN%%x*}"; fbh="${SCREEN#*x}"; fbh="${fbh%%x*}"
  wid="$(DISPLAY="$disp" "$XDO" search --name '^Ducktape$' 2>/dev/null | tail -1)"
  [ -n "$wid" ] || return 0
  DISPLAY="$disp" "$XDO" windowmove "$wid" 0 0 2>/dev/null
  DISPLAY="$disp" "$XDO" windowsize "$wid" "$fbw" "$fbh" 2>/dev/null
  DISPLAY="$disp" "$XDO" windowfocus "$wid" 2>/dev/null
}

# ── the one exposed web server: static console + token-routed VNC ──
up_web(){
  port_up "$WEB_PORT" && { log "web :$WEB_PORT already up"; return; }
  [ -f "$DIST/index.html" ] || { log "console not built — run '$0 build-console' first"; return 1; }
  setsid nohup bun "$SELF/fleet.mjs" serve "$DIST" "$TOKENS" "$TSIP" "$WEB_PORT" \
    >"$STATE/web.log" 2>&1 < /dev/null &
  echo $! >"$STATE/web.pid"
  sleep 2
  log "web :$WEB_PORT up (console + token router)"
}

# ── fleet.json (served at /fleet.json) ──────────────────
emit_json(){
  MAIN_ROOT="$MAIN_ROOT" BASE_BRANCH="$BASE_BRANCH" TSIP="$TSIP" WEB_PORT="$WEB_PORT" \
  DISP_BASE="$DISP_BASE" VNC_BASE="$VNC_BASE" \
  STATE="$STATE" DIST="$DIST" TAURI_AGENT_APP_ID="com.ducktape.app" \
  bun "$SELF/fleet.mjs" emit
}

# ── select worktrees from args (branch names) or all ────
selected(){
  if [ "$#" -eq 0 ]; then worktrees; return; fi
  local want=" $* "
  worktrees | while IFS=$'\t' read -r path br id; do
    case "$want" in *" $br "*|*" $id "*) printf '%s\t%s\t%s\n' "$path" "$br" "$id";; esac
  done
}

# ── commands ────────────────────────────────────────────
cmd="${1:-status}"; shift || true
case "$cmd" in
  up)
    echo "bringing up fleet…"
    while IFS=$'\t' read -r path br id; do up_one "$path" "$br" "$id"; done < <(selected "$@")
    # give apps a moment, then focus+fill each display
    sleep 3
    while IFS=$'\t' read -r path br id; do
      slot="$(slot_for "$id")"; focus_fill ":$((DISP_BASE+slot))"
    done < <(selected "$@")
    emit_json
    up_web
    echo; "$0" url ;;
  down)
    echo "tearing down…"
    while IFS=$'\t' read -r path br id; do
      slot="$(slot_for "$id")"; disp=":$((DISP_BASE+slot))"; vnc=$((VNC_BASE+slot))
      pkill -f "target/debug/ducktape-desktop" 2>/dev/null || true
      "$X11VNC" -R stop >/dev/null 2>&1 || true
      pkill -f "rfbport $vnc" 2>/dev/null || true
      pkill -f "Xvfb $disp " 2>/dev/null || true
      rm -f "$TOKENS/$id" "$STATE/$id/tauri-agent/com.ducktape.app/endpoint.json"
      log "[$id] stopped"
    done < <(selected "$@")
    if [ "$#" -eq 0 ]; then
      web_pid="$(cat "$STATE/web.pid" 2>/dev/null || true)"
      case "$(ps -p "$web_pid" -o args= 2>/dev/null)" in
        *"$SELF/fleet.mjs serve"*) kill "$web_pid" 2>/dev/null || true ;;
      esac
      rm -f "$STATE/web.pid"
      log "web stopped"
    fi
    emit_json ;;
  refresh) emit_json ;;
  build-console)
    ( cd "$CONSOLE_DIR" && BUN_INSTALL_CACHE_DIR="$REAL_HOME/.bun/install/cache" bun install && bun run build )
    log "console built -> $DIST" ;;
  status)
    echo "fleet status (base $BASE_BRANCH):"
    worktrees | while IFS=$'\t' read -r path br id; do
      slot="$(slot_for "$id")"; vnc=$((VNC_BASE+slot))
      up=$(port_up "$vnc" && echo up || echo down)
      printf '  %-28s slot %-2s vnc %-5s %s\n' "$br" "$slot" "$vnc" "$up"
    done
    port_up "$WEB_PORT" && log "web :$WEB_PORT listening" || log "web :$WEB_PORT down"
    echo; "$0" url ;;
  url)
    echo "dashboard (open on any tailnet device):"
    log "http://$TSIP:$WEB_PORT/" ;;
  *) echo "usage: $0 {up|down|status|refresh|build-console|url} [branch…]"; exit 1 ;;
esac
