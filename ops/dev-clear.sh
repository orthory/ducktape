#!/usr/bin/env bash
# make dev-clear — stop the background node and services left by `make dev`.
#
# This is the non-destructive twin of demo-clear: it preserves the workspace,
# registry entry, module state, wallets, and airlock credential store. It also
# leaves the foreground desktop app and the separate `make demo-app` server
# alone. Every process is selected by BOTH its ducktape node/service command
# shape and this exact workspace path before it may receive a signal.
set -uo pipefail

ID="${DEMO_WORKSPACE_ID:-demo}"
case "$ID" in
  ""|*/*|*..*|.*)
    printf '\033[31m[dev-clear] unsafe workspace id: %s\033[0m\n' "$ID" >&2
    exit 1
    ;;
esac
DUCK="${DUCKTAPE_HOME:-$HOME/.ducktape}"
WSDIR="$DUCK/workspaces/$ID"

log(){ printf '\033[36m[dev-clear]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[dev-clear] %s\033[0m\n' "$*" >&2; exit 1; }

# Candidate discovery is a `pgrep -f` sweep for the workspace path: nothing
# writes a pidfile, so the sweep is what finds a node left by a seed or an older
# dev loop. Admission is deliberately narrow: mentioning this workspace in an
# editor, shell, or diagnostic command is not enough.
managed_pids(){
  local pid executable command
  pgrep -f "$WSDIR" 2>/dev/null | while read -r pid; do
    [ -n "$pid" ] && [ "$pid" != "$$" ] || continue
    executable="$(ps -ww -p "$pid" -o comm= 2>/dev/null)"
    case "${executable##*/}" in
      ducktape) ;;
      *) continue ;;
    esac
    command="$(ps -p "$pid" -o command= 2>/dev/null)"
    case "$command" in
      *ducktape*" node run "*|*ducktape*" service run "*) ;;
      *) continue ;;
    esac
    case "$command" in
      *"$WSDIR"*) printf '%s\n' "$pid" ;;
    esac
  done
}

node_pids(){
  local pid
  managed_pids | while read -r pid; do
    case "$(ps -p "$pid" -o command= 2>/dev/null)" in
      *ducktape*" node run "*) printf '%s\n' "$pid" ;;
    esac
  done
}

join_pids(){
  xargs 2>/dev/null
}

INITIAL="$(managed_pids | join_pids)"
if [ -z "$INITIAL" ]; then
  log "nothing to stop for '$ID'"
  exit 0
fi

# Ask the node to stop through its own operator surface first. A missing or
# refused admin credential is diagnostic, not a reason to leave the dev loop
# behind: the exact-command PID sweep below still terminates only this
# workspace's processes.
if [ -n "$(node_pids)" ]; then
  LISTEN=""
  if [ -f "$WSDIR/node.toml" ]; then
    LISTEN="$(sed -n 's/^[[:space:]]*http_listen[[:space:]]*=[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
      "$WSDIR/node.toml" | head -1 | tr -d '[:space:]')"
  fi
  if [ -z "$LISTEN" ]; then
    log "no http endpoint in $WSDIR/node.toml — using the PID sweep"
  elif [ ! -r "$WSDIR/admin.token" ]; then
    log "no readable admin token — using the PID sweep"
  else
    CODE="$(curl -s -o /dev/null -w '%{http_code}' -m 2 \
      -X POST "http://$LISTEN/v1/admin/shutdown" \
      -H "x-ducktape-admin-token: $(<"$WSDIR/admin.token")" 2>/dev/null)"
    case "$CODE" in
      2*) log "node accepted graceful shutdown" ;;
      *) log "graceful shutdown did not succeed (http ${CODE:-none}) — using the PID sweep" ;;
    esac
    for _ in $(seq 1 20); do [ -z "$(node_pids)" ] && break; sleep 0.1; done
  fi
fi

PIDS="$(managed_pids | join_pids)"
if [ -n "$PIDS" ]; then
  log "stopping dev process(es): $PIDS"
  # shellcheck disable=SC2086
  kill -TERM $PIDS 2>/dev/null
  for _ in $(seq 1 50); do [ -z "$(managed_pids)" ] && break; sleep 0.1; done
fi

REMAIN="$(managed_pids | join_pids)"
if [ -n "$REMAIN" ]; then
  log "forcing dev process(es) that ignored SIGTERM: $REMAIN"
  # shellcheck disable=SC2086
  kill -KILL $REMAIN 2>/dev/null
  for _ in $(seq 1 20); do [ -z "$(managed_pids)" ] && break; sleep 0.1; done
fi

[ -z "$(managed_pids)" ] || die "a '$ID' dev process is still running"
if [ -d "$WSDIR" ]; then
  log "preserved $WSDIR"
fi
printf '\033[32m[dev-clear] done.\033[0m\n'
