#!/usr/bin/env bash
# Guards the one line in demo-clear.sh that can silently rot: what it prints
# when the node REFUSES /v1/admin/shutdown. The reason token there must be the
# node's own (`refuse` in crates/noded/src/admin.rs puts it in the response
# body) — a token invented in the script greps to nothing, which is how three
# fictional ones lived there until #1331.
#
# So the stub answers with a reason and an error sentence NO script could
# plausibly hardcode. That IS the test: were the stub to reply with a real token
# like `operator_token_mismatch`, a script that hardcoded that string for 403
# would pass — which is precisely the defect this guards against.
#
# Drives the real script against a stub admin surface under a throwaway HOME
# and a workspace id no human uses, so the only state it can delete is its own.
set -uo pipefail

OPS="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# demo-clear.sh refuses to run without bun, so a box without it has nothing to
# test here — skip with a notice rather than failing the whole `make test` gate,
# the way its podman lines are tolerant of a host that has no podman.
command -v bun >/dev/null || { printf '[demo-clear-test] skipped — bun is not installed\n' >&2; exit 0; }

ID="demo-clear-selftest"
REASON="demo_clear_selftest_9f21"
ERROR="the stub refused; only the response body knows why"
TMP="$(mktemp -d)"
WS="$TMP/.ducktape/workspaces/$ID"
# run_case is always called inside a command substitution, so a variable it sets
# never reaches this shell — the stub's pid goes through a file the trap reads.
# Nothing leaks on the normal path (run_case reaps its own stub); this is for a
# signal landing mid-case.
cleanup(){
  [ -r "$TMP/stub.pid" ] && kill "$(cat "$TMP/stub.pid")" 2>/dev/null
  rm -rf "$TMP"
}
trap cleanup EXIT

fail(){ printf '\033[31m[demo-clear-test] %s\033[0m\n' "$*" >&2; exit 1; }

# Run demo-clear against a stub that answers every request with this status and
# body. The stub prints its port once `Bun.serve` is LISTENING, and the read
# blocks on that line — the handshake is the event, not a sleep.
run_case(){
  local status="$1" body="$2" port out pid
  rm -rf "$TMP/.ducktape" "$TMP/ready"
  mkdir -p "$WS"
  printf 'stub-token\n' > "$WS/admin.token"
  mkfifo "$TMP/ready"
  bun -e '
    const [status, body] = process.argv.slice(1);
    const server = Bun.serve({ port: 0, fetch: () => new Response(body, { status: Number(status) }) });
    console.log(server.port);
  ' "$status" "$body" > "$TMP/ready" 2>/dev/null &
  pid=$!
  printf '%s\n' "$pid" > "$TMP/stub.pid"
  # the timeout is not a wait-for-readiness poll — the fifo line is the event.
  # It is only so a stub that never listens fails this gate instead of hanging
  # `make test` forever.
  read -r -t 10 port < "$TMP/ready" || fail "the stub never reported a port"
  printf '{"active":"%s","workspaces":[{"id":"%s","ports":{"http":%s}}]}\n' "$ID" "$ID" "$port" \
    > "$TMP/.ducktape/registry.json"
  out="$(HOME="$TMP" DEMO_WORKSPACE_ID="$ID" bash "$OPS/demo-clear.sh" 2>&1)"
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null; rm -f "$TMP/stub.pid"
  printf '%s\n' "$out"
}

# 1. a refusal that names itself: the script must print THAT token and THAT
#    sentence, verbatim — neither is guessable from the 403 alone.
REFUSED="$(run_case 403 "{\"error\":\"$ERROR\",\"reason\":\"$REASON\"}")"
case "$REFUSED" in
  *"reason=$REASON"*) ;;
  *) printf '%s\n' "$REFUSED" >&2; fail "the refusal line dropped the node's reason token" ;;
esac
case "$REFUSED" in
  *"$ERROR"*) ;;
  *) printf '%s\n' "$REFUSED" >&2; fail "the refusal line dropped the node's error sentence" ;;
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
