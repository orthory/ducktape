#!/usr/bin/env bash
# make demo-clear — remove the "demo" workspace that demo-seed created.
#
# Stops the node and the service daemons still serving the workspace (graceful
# /v1/admin/shutdown first, then a sweep that admits only `ducktape node run`
# and `ducktape service run` processes whose command line names the workspace
# dir — a recycled pid, or an editor open on a log in here, must never be
# taken down), then deletes <ducktape home>/<id> — the whole network: its
# config, keystore, guest images, executors and state. Other workspaces under
# the home are untouched. The home is $DUCKTAPE_HOME when set, else
# ~/.ducktape.
#
# If `make demo-app` is still running it keeps serving its loopback port — it's
# a plain foreground process you own; Ctrl-C it yourself. The route it served
# dies with the workspace either way.
set -uo pipefail

ID="${DEMO_WORKSPACE_ID:-demo}"
# this script kills by path match and rm -rfs the workspace dir — refuse an id
# that could walk WSDIR out of the home (e.g. "../..").
case "$ID" in ""|*/*|*..*|.*) printf '\033[31m[demo-clear] unsafe workspace id: %s\033[0m\n' "$ID" >&2; exit 1;; esac
# the SAME root demo-seed wrote into. Hardcoding $HOME here made the
# documented inverse of `make demo-seed` report "no demo workspace" and
# aim its rm -rf at a root the seed never touched.
DUCK="${DUCKTAPE_HOME:-$HOME/.ducktape}"
WSDIR="$DUCK/$ID"

log(){ printf '\033[36m[demo-clear]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-clear] %s\033[0m\n' "$*" >&2; exit 1; }

# One string field out of the node's flat `{"error":…,"reason":…}` refusal body.
# `reason` is a snake_case token, so `[^"]*` is exact for it; the `error`
# sentence would truncate at an escaped quote, which is fine for a log line.
json_string(){ sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }

# The loopback of a node.toml `http_listen`'s address family, port kept: the
# bind is a wildcard by default, which no client can dial as written.
loopback_base(){
  local port="${1##*:}" host="${1%:*}"
  case "$host" in
    \[*) printf '[::1]:%s' "$port" ;;
    *) printf '127.0.0.1:%s' "$port" ;;
  esac
}

# pids of LIVE ducktape processes serving THIS workspace: the node
# (`ducktape node run`) and the service daemons (`ducktape service run`) whose
# command line names the workspace dir. Nothing writes a pidfile — a node is
# started by hand, by a seed run or by `make dev`, and the app only ever PRINTS
# that command — so a `pgrep -f` sweep for the workspace dir is the discovery.
# Admission is deliberately narrow: mentioning this workspace in an editor,
# shell, or diagnostic command is not enough.
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

# ── 1. anything to clear? ──────────────────────────────────────
if [ ! -d "$WSDIR" ]; then
  log "nothing to clear — no '$ID' workspace under $DUCK"
  exit 0
fi

# ── 2. stop the workspace's node, graceful first ───────────────
# the node's app endpoint, from the workspace's own config — the one place it
# is written.
LISTEN=""
if [ -f "$WSDIR/node.toml" ]; then
  LISTEN="$(sed -n 's/^[[:space:]]*http_listen[[:space:]]*=[[:space:]]*"\{0,1\}\([^"#]*\)"\{0,1\}.*/\1/p' \
    "$WSDIR/node.toml" | head -1 | tr -d '[:space:]')"
fi
# SAY SO on every path that does not gracefully stop the node. Falling silently
# through to the SIGTERM sweep looks EXACTLY like a graceful stop that worked,
# and hides which of these happened: no token on disk (DUCKTAPE_ADMIN=off mints
# none; another uid's node writes one we cannot read), or a node under
# DUCKTAPE_ADMIN=public, where the operator token is not the credential at all
# — that wants an owner PoP from `ducktape user sign-admin`, which needs the
# user key's password and is more than this script should carry.
# The sweep below still stops the node either way; these lines are why it took a
# signal to do it.
if [ -z "$LISTEN" ]; then
  log "no http endpoint in $WSDIR/node.toml — using the pid sweep"
elif [ ! -r "$WSDIR/admin.token" ]; then
  log "no readable $WSDIR/admin.token — using the pid sweep"
else
  # /v1/admin/* is the OPERATOR's plane under the default loopback exposure:
  # loopback presence is not authority (a service daemon is a loopback peer
  # too), so the request carries the credential the node minted 0600 into its
  # own workspace. Capture the STATUS and the BODY — discarding them is what
  # made a refusal indistinguishable from a stop, and the node NAMES its own
  # refusal in that body (`{"error":…,"reason":…}`, crates/noded/src/admin.rs).
  # Print that token verbatim: a reason invented here greps to nothing.
  RESPONSE="$(curl -s -m 2 -w '\n%{http_code}' \
    -X POST "http://$(loopback_base "$LISTEN")/v1/admin/shutdown" \
    -H "x-ducktape-admin-token: $(cat "$WSDIR/admin.token")" 2>/dev/null)"
  CODE="${RESPONSE##*$'\n'}"
  # BOTH fields the node sent: `reason` is the greppable token and `error` is
  # `AdminRefusal::message()` — the operator-facing sentence that says what to do
  # about it ("re-read admin.token from the node's workspace; a restart mints a
  # new one"). A route the node never mounted (admin disabled) 404s with an empty
  # body, and an unreachable port answers 000 with none — then the status is the
  # whole diagnosis and the line carries neither.
  BODY="${RESPONSE%$'\n'*}"
  REASON="$(printf '%s' "$BODY" | json_string reason)"
  SENTENCE="$(printf '%s' "$BODY" | json_string error)"
  DETAIL="${REASON:+reason=$REASON}${SENTENCE:+${REASON:+ }$SENTENCE}"
  case "$CODE" in
    2*) : ;;
    *) log "graceful shutdown did not succeed (http ${CODE:-none}${DETAIL:+, $DETAIL}) — using the pid sweep" ;;
  esac
fi

PIDS="$(managed_pids | xargs)"
if [ -n "$PIDS" ]; then
  log "stopping node/service process(es): $PIDS"
  # shellcheck disable=SC2086
  kill -TERM $PIDS 2>/dev/null
  for _ in $(seq 1 50); do [ -z "$(managed_pids)" ] && break; sleep 0.1; done
  REMAIN="$(managed_pids | xargs)"
  if [ -n "$REMAIN" ]; then
    # shellcheck disable=SC2086
    kill -KILL $REMAIN 2>/dev/null
    for _ in $(seq 1 20); do [ -z "$(managed_pids)" ] && break; sleep 0.1; done
  fi
fi
# the honest gate: never delete state a live process would just re-create.
[ -z "$(managed_pids)" ] || die "a '$ID' node or service is still running and could not be stopped — stop it manually, then re-run"

# ── 3. delete the workspace dir ────────────────────────────────
# A plain rm is enough. This used to need a `podman unshare` pass to unmount
# a container storage overlay left under the workspace; a run's storage is a
# microVM's own block device, so there is nothing under here mounted in
# another user namespace. The directory IS the network's whole footprint on
# this box: nothing outside it lists or names the workspace.
rm -rf "$WSDIR" 2>/dev/null
[ ! -d "$WSDIR" ] || die "could not delete $WSDIR — stop its services and remove the remaining files"
log "deleted $WSDIR"

printf '\033[32m[demo-clear] done.\033[0m\n'
