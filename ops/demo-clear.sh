#!/usr/bin/env bash
# make demo-clear — remove the "demo" workspace that demo-seed created.
#
# Stops any node still serving the workspace (graceful /v1/admin/shutdown first,
# then a pgrep sweep where every candidate's command line is verified against
# the workspace dir before it may be killed — a recycled pid must never take an
# innocent process down), deletes <ducktape home>/workspaces/<id>, and
# drops the entry from <ducktape home>/registry.json, handing "active" to
# another workspace when the demo held it. Other workspaces are untouched.
# The home is $DUCKTAPE_HOME when set, else ~/.ducktape.
#
# If `make demo-app` is still running it keeps serving its loopback port — it's
# a plain foreground process you own; Ctrl-C it yourself. The route it served
# dies with the workspace either way.
set -uo pipefail

ID="${DEMO_WORKSPACE_ID:-demo}"
# this script kills by path match and rm -rfs the workspace dir — refuse an id
# that could walk WSDIR out of the workspaces root (e.g. "../..").
case "$ID" in ""|*/*|*..*|.*) printf '\033[31m[demo-clear] unsafe workspace id: %s\033[0m\n' "$ID" >&2; exit 1;; esac
# the SAME root demo-seed wrote into. Hardcoding $HOME here made the
# documented inverse of `make demo-seed` report "no demo workspace" and
# aim its rm -rf at a root the seed never touched.
DUCK="${DUCKTAPE_HOME:-$HOME/.ducktape}"
WSDIR="$DUCK/workspaces/$ID"
REG="$DUCK/registry.json"

log(){ printf '\033[36m[demo-clear]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-clear] %s\033[0m\n' "$*" >&2; exit 1; }

# One string field out of the node's flat `{"error":…,"reason":…}` refusal body.
# `reason` is a snake_case token, so `[^"]*` is exact for it; the `error`
# sentence would truncate at an escaped quote, which is fine for a log line.
json_string(){ sed -n "s/.*\"$1\":\"\([^\"]*\)\".*/\1/p"; }

command -v bun >/dev/null || die "bun is required"

# pids of LIVE processes verifiably serving THIS workspace: a `pgrep -f` sweep
# for the workspace dir. Nothing writes a pidfile — a node is started by hand
# (`ducktape node run`) or by a seed run, and the app only ever PRINTS that
# command — so the sweep is the whole discovery. Every candidate's command line
# is checked before it may be killed.
node_pids(){
  pgrep -f "$WSDIR" 2>/dev/null | while read -r pid; do
    [ -n "$pid" ] && [ "$pid" != "$$" ] || continue
    case "$(ps -p "$pid" -o command= 2>/dev/null)" in
      *"$WSDIR"*) printf '%s\n' "$pid" ;;
    esac
  done
}

# ── 1. anything to clear? ──────────────────────────────────────
IN_REGISTRY="$(bun - "$REG" "$ID" 2>/dev/null <<'JS'
import { existsSync, readFileSync } from "node:fs";
const [path, id] = process.argv.slice(2);
if (!existsSync(path)) process.exit(0);
try {
  const registry = JSON.parse(readFileSync(path, "utf8"));
  const listed = (registry.workspaces ?? []).some((item) => item.id === id);
  if (listed || registry.active === id) console.log("yes");
} catch {}
JS
)"
if [ ! -d "$WSDIR" ] && [ -z "$IN_REGISTRY" ]; then
  log "nothing to clear — no '$ID' workspace on disk or in the registry"
  exit 0
fi

# ── 2. stop the workspace's node, graceful first ───────────────
HTTP_PORT="$(bun - "$REG" "$ID" 2>/dev/null <<'JS'
import { readFileSync } from "node:fs";
const [path, id] = process.argv.slice(2);
try {
  const port = JSON.parse(readFileSync(path, "utf8")).workspaces
    ?.find((item) => item.id === id)?.ports?.http;
  if (Number.isInteger(port) && port > 0) console.log(port);
} catch {}
JS
)"
# SAY SO on every path that does not gracefully stop the node. Falling silently
# through to the SIGTERM sweep looks EXACTLY like a graceful stop that worked,
# and hides which of these happened: no token on disk (DUCKTAPE_ADMIN=off mints
# none; another uid's node writes one we cannot read), or a node under
# DUCKTAPE_ADMIN=public, where the operator token is not the credential at all
# — that wants an owner PoP from `ducktape user sign-admin`, which needs the
# user key's password and is more than this script should carry.
# The sweep below still stops the node either way; these lines are why it took a
# signal to do it.
if [ -z "$HTTP_PORT" ]; then
  log "no http port in the registry for '$ID' — using the pid sweep"
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
    -X POST "http://127.0.0.1:$HTTP_PORT/v1/admin/shutdown" \
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

PIDS="$(node_pids | xargs)"
if [ -n "$PIDS" ]; then
  log "stopping node process(es): $PIDS"
  # shellcheck disable=SC2086
  kill -TERM $PIDS 2>/dev/null
  for _ in $(seq 1 50); do [ -z "$(node_pids)" ] && break; sleep 0.1; done
  REMAIN="$(node_pids | xargs)"
  if [ -n "$REMAIN" ]; then
    # shellcheck disable=SC2086
    kill -KILL $REMAIN 2>/dev/null
    for _ in $(seq 1 20); do [ -z "$(node_pids)" ] && break; sleep 0.1; done
  fi
fi
# the honest gate: never delete state a live process would just re-create.
[ -z "$(node_pids)" ] || die "a '$ID' node is still running and could not be stopped — stop it manually, then re-run"

# ── 3. delete the workspace dir ────────────────────────────────
if [ -d "$WSDIR" ]; then
  # A plain rm is enough now. This used to need a `podman unshare` pass to
  # unmount a container storage overlay left under the workspace; a run's
  # storage is a microVM's own block device, so there is nothing under here
  # mounted in another user namespace.
  rm -rf "$WSDIR" 2>/dev/null
  [ ! -d "$WSDIR" ] || die "could not delete $WSDIR — stop its services and remove the remaining files"
  log "deleted $WSDIR"
fi

# ── 4. drop it from the registry (other workspaces untouched) ──
if [ -n "$IN_REGISTRY" ]; then
  NEXT="$(bun - "$REG" "$ID" <<'JS'
import { readFileSync, writeFileSync } from "node:fs";
const [path, id] = process.argv.slice(2);
const registry = JSON.parse(readFileSync(path, "utf8"));
registry.workspaces = (registry.workspaces ?? []).filter((item) => item.id !== id);
if (registry.active === id) registry.active = registry.workspaces[0]?.id ?? null;
writeFileSync(path, JSON.stringify(registry, null, 2));
console.log(registry.active ?? "none");
JS
)" || die "registry update failed — $REG"
  log "removed '$ID' from the registry (active workspace: $NEXT)"
fi

printf '\033[32m[demo-clear] done.\033[0m\n'
