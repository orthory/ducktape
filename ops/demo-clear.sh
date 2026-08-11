#!/usr/bin/env bash
# make demo-clear — remove the "demo" workspace that demo-seed created.
#
# Stops any node still serving the workspace (graceful /v1/admin/shutdown first, then
# a pidfile + pgrep sweep where every candidate's command line is verified
# against the workspace dir before it may be killed — a recycled pid must never
# take an innocent process down), deletes ~/.ducktape/workspaces/<id>, and
# drops the entry from ~/.ducktape/registry.json, handing "active" to another
# workspace when the demo held it. Other workspaces are untouched.
#
# If `make demo-app` is still running it keeps serving its loopback port — it's
# a plain foreground process you own; Ctrl-C it yourself. The route it served
# dies with the workspace either way.
set -uo pipefail

ID="${DEMO_WORKSPACE_ID:-demo}"
# this script kills by path match and rm -rfs the workspace dir — refuse an id
# that could walk WSDIR out of ~/.ducktape/workspaces (e.g. "../..").
case "$ID" in ""|*/*|*..*|.*) printf '\033[31m[demo-clear] unsafe workspace id: %s\033[0m\n' "$ID" >&2; exit 1;; esac
DUCK="$HOME/.ducktape"
WSDIR="$DUCK/workspaces/$ID"
REG="$DUCK/registry.json"

log(){ printf '\033[36m[demo-clear]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-clear] %s\033[0m\n' "$*" >&2; exit 1; }

command -v bun >/dev/null || die "bun is required"

# pids of LIVE processes verifiably serving THIS workspace: the pidfile pid the
# app records on spawn plus a `pgrep -f` sweep for the workspace dir (a seed
# run's node leaves no pidfile; the sweep still finds it). Every candidate's
# command line is checked before it may be killed.
node_pids(){
  {
    [ -f "$WSDIR/node.pid" ] && printf '%s\n' "$(cat "$WSDIR/node.pid")"
    pgrep -f "$WSDIR" 2>/dev/null || true
  } | sort -un 2>/dev/null | while read -r pid; do
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
# and hides which of three quite different things happened: no token on disk
# (DUCKTAPE_ADMIN=off mints none; another uid's node writes one we cannot read),
# or a node under DUCKTAPE_ADMIN=public, where the operator token is not the
# credential at all — that wants an owner PoP from `ducktape user sign-admin`,
# which needs the user key's password and is more than this script should carry.
# The sweep below still stops the node either way; these lines are why it took a
# signal to do it.
if [ -z "$HTTP_PORT" ]; then
  log "no http port in the registry for '$ID' — using the pid sweep"
elif [ ! -r "$WSDIR/admin.token" ]; then
  log "no readable $WSDIR/admin.token — reason=operator_token_unreadable, using the pid sweep"
else
  # /v1/admin/* is the OPERATOR's plane under the default loopback exposure:
  # loopback presence is not authority (a service daemon is a loopback peer
  # too), so the request carries the credential the node minted 0600 into its
  # own workspace. Capture the STATUS — discarding it is what made a refusal
  # indistinguishable from a stop.
  CODE="$(curl -s -o /dev/null -w '%{http_code}' -m 2 \
    -X POST "http://127.0.0.1:$HTTP_PORT/v1/admin/shutdown" \
    -H "x-ducktape-admin-token: $(cat "$WSDIR/admin.token")" 2>/dev/null)"
  case "$CODE" in
    2*) : ;;
    401) log "admin refused the operator token (401) — reason=wrong_credential_type, this node wants an owner PoP (DUCKTAPE_ADMIN=public); using the pid sweep" ;;
    403) log "admin refused the operator token (403) — reason=operator_token_mismatch, the node restarted since this token was written; using the pid sweep" ;;
    404) log "no admin namespace on this node (404) — reason=admin_disabled, using the pid sweep" ;;
    *)   log "graceful shutdown did not succeed (http ${CODE:-none}) — using the pid sweep" ;;
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
  rm -rf "$WSDIR"
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
