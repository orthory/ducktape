#!/usr/bin/env bash
# make demo-clear — remove the "demo" workspace that demo-seed created.
#
# Stops any node still serving the workspace (graceful /v1/shutdown first, then
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
if [ -n "$HTTP_PORT" ]; then
  curl -s -m 2 -X POST "http://127.0.0.1:$HTTP_PORT/v1/shutdown" >/dev/null 2>&1
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
