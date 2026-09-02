#!/usr/bin/env bash
# Guards the one line in demo-clear.sh that can silently rot: what it prints
# when the node REFUSES /v1/admin/shutdown. The reason token there must be the
# node's own (`refuse` in crates/noded/src/admin.rs puts it in the response
# body) — a token invented in the script greps to nothing, which is how three
# fictional ones lived there until #1331.
#
# Drives the real script against a stub admin surface under a throwaway HOME
# and a workspace id no human uses, so the only state it can delete is its own.
set -uo pipefail

OPS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
command -v bun >/dev/null || { printf '[demo-clear-test] bun is required\n' >&2; exit 1; }

ID="demo-clear-selftest"
TMP="$(mktemp -d)"
WS="$TMP/.ducktape/workspaces/$ID"
STUB=""
cleanup(){ [ -n "$STUB" ] && kill "$STUB" 2>/dev/null; rm -rf "$TMP"; }
trap cleanup EXIT

fail(){ printf '\033[31m[demo-clear-test] %s\033[0m\n' "$*" >&2; exit 1; }

# Run demo-clear against a stub that answers every request with this status and
# body. The stub prints its port once `Bun.serve` is LISTENING, and the read
# blocks on that line — the handshake is the event, not a sleep.
run_case(){
  local status="$1" body="$2" port out
  rm -rf "$TMP/.ducktape" "$TMP/ready"
  mkdir -p "$WS"
  printf 'stub-token\n' > "$WS/admin.token"
  mkfifo "$TMP/ready"
  bun -e '
    const [status, body] = process.argv.slice(1);
    const server = Bun.serve({ port: 0, fetch: () => new Response(body, { status: Number(status) }) });
    console.log(server.port);
  ' "$status" "$body" > "$TMP/ready" 2>/dev/null &
  STUB=$!
  read -r port < "$TMP/ready"
  printf '{"active":"%s","workspaces":[{"id":"%s","ports":{"http":%s}}]}\n' "$ID" "$ID" "$port" \
    > "$TMP/.ducktape/registry.json"
  out="$(HOME="$TMP" DEMO_WORKSPACE_ID="$ID" bash "$OPS/demo-clear.sh" 2>&1)"
  kill "$STUB" 2>/dev/null; wait "$STUB" 2>/dev/null; STUB=""
  printf '%s\n' "$out"
}

# 1. a refusal that names itself: the script must print THAT token, verbatim.
REFUSED="$(run_case 403 '{"error":"that operator credential is not this node@s","reason":"operator_token_mismatch"}')"
case "$REFUSED" in
  *"reason=operator_token_mismatch"*) ;;
  *) printf '%s\n' "$REFUSED" >&2; fail "the refusal line dropped the node's reason token" ;;
esac
[ -d "$WS" ] && fail "the workspace survived a clear"

# 2. no admin namespace at all: axum 404s with an empty body, so there is no
#    reason to print and the script must not invent one.
ABSENT="$(run_case 404 '')"
case "$ABSENT" in
  *"http 404"*) ;;
  *) printf '%s\n' "$ABSENT" >&2; fail "the refusal line dropped the status" ;;
esac
case "$ABSENT" in
  *"reason="*) printf '%s\n' "$ABSENT" >&2; fail "a body with no reason must not produce one" ;;
esac

printf '\033[32m[demo-clear-test] ok\033[0m\n'
