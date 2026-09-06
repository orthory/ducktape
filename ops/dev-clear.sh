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

# One string field out of the node's flat `{"error":…,"reason":…}` refusal body.
# `reason` is a snake_case token, so `[^"]*` is exact for it; the `error`
# sentence would truncate at an escaped quote, which is fine for a log line.
json_string(){ sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }

# The loopback of a node.toml `http_listen`'s address family, port kept. This
# script dials its OWN node from this box, and the bind is a wildcard by
# default (`0.0.0.0`, or `[::]`) that no client can dial as written — the same
# rewrite every co-located process applies (`workspace_config::http_base_of`).
# The operator credential presented below is honored only from a loopback peer.
loopback_base(){
  local port="${1##*:}" host="${1%:*}"
  case "$host" in
    \[*) printf '[::1]:%s' "$port" ;;
    *) printf '127.0.0.1:%s' "$port" ;;
  esac
}

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
    # Capture the BODY as well as the status: the node NAMES its own refusal
    # there (`{"error":…,"reason":…}`, crates/noded/src/admin.rs), and throwing
    # it away is what made a refusal indistinguishable from a stop. The token is
    # printed verbatim — one invented here greps to nothing.
    RESPONSE="$(curl -s -m 2 -w '\n%{http_code}' \
      -X POST "http://$(loopback_base "$LISTEN")/v1/admin/shutdown" \
      -H "x-ducktape-admin-token: $(<"$WSDIR/admin.token")" 2>/dev/null)"
    CODE="${RESPONSE##*$'\n'}"
    BODY="${RESPONSE%$'\n'*}"
    REASON="$(printf '%s' "$BODY" | json_string reason)"
    SENTENCE="$(printf '%s' "$BODY" | json_string error)"
    # A route the node never mounted 404s with an empty body and an unreachable
    # port answers 000 with none — then the status is the whole diagnosis.
    DETAIL="${REASON:+reason=$REASON}${SENTENCE:+${REASON:+ }$SENTENCE}"
    case "$CODE" in
      2*) log "node accepted graceful shutdown" ;;
      *) log "graceful shutdown refused (http ${CODE:-none})${DETAIL:+: $DETAIL} — using the PID sweep" ;;
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
