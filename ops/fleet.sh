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
# Userspace only (no root). Reuses the x11vnc/xdotool/noVNC/websockify staged by
# remote-tauri under ~/.local/opt/remote-tauri. Port bases are offset from the
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
X11VNC="$PREFIX/root/usr/bin/x11vnc"
XDO="$PREFIX/root/usr/bin/xdotool"
NOVNC="$PREFIX/noVNC"
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
slot_for(){ python3 - "$STATE/slots.json" "$1" <<'PY'
import json, os, sys
path, idv = sys.argv[1], sys.argv[2]
try:    d = json.load(open(path))
except Exception: d = {}
if idv not in d:
    used = set(d.values())
    i = 0
    while i in used: i += 1
    d[idv] = i
    os.makedirs(os.path.dirname(path), exist_ok=True)
    json.dump(d, open(path, "w"))
print(d[idv])
PY
}

# ── bring up ONE worktree instance ──────────────────────
up_one(){
  local path="$1" branch="$2" id="$3"
  local slot; slot="$(slot_for "$id")"
  local disp=":$((DISP_BASE+slot))" vite=$((VITE_BASE+slot)) vnc=$((VNC_BASE+slot))
  local mcp="/tmp/tauri-mcp-$id.sock" home="$STATE/$id/home" wsdir="$STATE/$id"
  local app="$path/app"
  mkdir -p "$home" "$wsdir"
  log "[$id] slot $slot  disp $disp  vite $vite  vnc $vnc"

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

  # a dead instance leaves a stale socket file that would block restart; if the
  # VNC (started after the app) is gone, the instance is dead — clear it.
  if [ -S "$mcp" ] && ! port_up "$vnc"; then rm -f "$mcp"; fi

  # the app — isolated HOME, warm build caches, headless WebKit flags
  if ! [ -S "$mcp" ]; then
    ( cd "$app" && \
      HOME="$home" \
      CARGO_HOME="$REAL_HOME/.cargo" RUSTUP_HOME="$REAL_HOME/.rustup" \
      XDG_CACHE_HOME="$REAL_HOME/.cache" BUN_INSTALL_CACHE_DIR="$REAL_HOME/.bun/install/cache" \
      PATH="$REAL_HOME/.local/bin:$PATH" \
      DISPLAY="$disp" WEBKIT_DISABLE_DMABUF_RENDERER=1 WEBKIT_DISABLE_COMPOSITING_MODE=1 \
      LIBGL_ALWAYS_SOFTWARE=1 GDK_BACKEND=x11 \
      DUCKTAPE_TAURI_DEV_PORT="$vite" DUCKTAPE_TAURI_MCP_SOCKET="$mcp" \
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
  ( cd "$NOVNC/utils/websockify" && setsid nohup python3 -m websockify \
      --web "$DIST" \
      --token-plugin websockify.token_plugins.TokenFile --token-source "$TOKENS" \
      "$TSIP:$WEB_PORT" >"$STATE/websockify.log" 2>&1 < /dev/null & )
  sleep 2
  log "web :$WEB_PORT up (console + token router)"
}

# ── fleet.json (served at /fleet.json) ──────────────────
emit_json(){
  MAIN_ROOT="$MAIN_ROOT" BASE_BRANCH="$BASE_BRANCH" TSIP="$TSIP" WEB_PORT="$WEB_PORT" \
  DISP_BASE="$DISP_BASE" VITE_BASE="$VITE_BASE" VNC_BASE="$VNC_BASE" \
  STATE="$STATE" DIST="$DIST" \
  python3 - <<'PY'
import json, os, socket, subprocess, datetime, re
env = os.environ
main, base = env["MAIN_ROOT"], env["BASE_BRANCH"]
tsip, web = env["TSIP"], int(env["WEB_PORT"])
disp_b, vite_b, vnc_b = int(env["DISP_BASE"]), int(env["VITE_BASE"]), int(env["VNC_BASE"])
state, dist = env["STATE"], env["DIST"]

def slug(s): return re.sub(r"-+","-", re.sub(r"[^a-z0-9]","-", s.lower())).strip("-")
def sh(*a):
    try: return subprocess.run(a, capture_output=True, text=True, timeout=8).stdout.strip()
    except Exception: return ""
def port_open(p):
    s = socket.socket(); s.settimeout(0.3)
    try: return s.connect_ex(("127.0.0.1", p)) == 0
    finally: s.close()

try: slots = json.load(open(os.path.join(state, "slots.json")))
except Exception: slots = {}

# discover worktrees
raw = sh("git", "-C", main, "worktree", "list", "--porcelain")
wts, cur = [], {}
for line in raw.splitlines():
    if line.startswith("worktree "): cur = {"path": line[9:]}
    elif line.startswith("branch "): cur["branch"] = line[7:].replace("refs/heads/", "")
    elif line == "detached": cur["branch"] = None
    elif line == "" and cur:
        wts.append(cur); cur = {}
if cur: wts.append(cur)

out = []
for w in wts:
    path, branch = w.get("path"), w.get("branch")
    if not branch or not os.path.isdir(os.path.join(path, "app")): continue
    wid = slug(branch)
    sha = sh("git", "-C", path, "rev-parse", "--short", "HEAD")
    subj = sh("git", "-C", path, "log", "-1", "--pretty=%s")
    ahead = sh("git", "-C", path, "rev-list", "--count", f"{base}..HEAD") or "0"
    behind = sh("git", "-C", path, "rev-list", "--count", f"HEAD..{base}") or "0"
    # agent activity: what's been done to this branch. \x1f is a field sep the
    # commit subject can't contain. dirty = in-progress edits (the live pulse).
    commits = []
    for ln in sh("git", "-C", path, "log", "-4", "--pretty=%h\x1f%s\x1f%cr").splitlines():
        p = ln.split("\x1f")
        if len(p) == 3:
            commits.append({"sha": p[0], "subject": p[1], "age": p[2]})
    dirty = len([l for l in sh("git", "-C", path, "status", "--porcelain").splitlines() if l.strip()])
    node = {"id": wid, "branch": branch, "path": os.path.relpath(path, main),
            "head": {"sha": sha, "subject": subj},
            "parent": base if branch != base else None,
            "ahead": int(ahead or 0), "behind": int(behind or 0),
            "activity": {"dirty": dirty, "commits": commits},
            "status": "down"}
    if wid in slots:
        slot = slots[wid]
        vnc = vnc_b + slot
        node.update({"slot": slot, "display": f":{disp_b+slot}", "vncPort": vnc,
                     "token": wid})
        # "up" means the APP is live (its tauri-mcp socket exists) AND reachable
        # over VNC — a bare x11vnc on an empty Xvfb is not "up".
        sock = f"/tmp/tauri-mcp-{wid}.sock"
        if os.path.exists(sock) and port_open(vnc): node["status"] = "up"
        elif os.path.isfile(os.path.join(state, wid, "tauri.log")): node["status"] = "building"
        else: node["status"] = "down"
    out.append(node)

# base at the front; then by slot then branch
out.sort(key=lambda n: (n["branch"] != base, n.get("slot", 999), n["branch"]))
doc = {"generatedAt": datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds"),
       "host": tsip, "webPort": web, "base": base, "worktrees": out}
os.makedirs(dist, exist_ok=True)
json.dump(doc, open(os.path.join(dist, "fleet.json"), "w"), indent=2)
print(f"fleet.json: {len(out)} worktree(s) -> {os.path.join(dist,'fleet.json')}")
PY
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
      pkill -f "tauri-mcp-$id.sock" 2>/dev/null || true
      "$X11VNC" -R stop >/dev/null 2>&1 || true
      pkill -f "rfbport $vnc" 2>/dev/null || true
      pkill -f "Xvfb $disp " 2>/dev/null || true
      rm -f "$TOKENS/$id" "/tmp/tauri-mcp-$id.sock"
      log "[$id] stopped"
    done < <(selected "$@")
    [ "$#" -eq 0 ] && { pkill -f 'python3 -m websockify' 2>/dev/null || true; log "web stopped"; }
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
