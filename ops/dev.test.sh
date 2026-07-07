#!/usr/bin/env bash
# Unit tests for ops/dev.sh's honesty logic. Sources dev.sh with DEV_SH_LIB=1 so
# only the functions load (main is skipped), then drives them with stub nodes.
# Run: bash ops/dev.test.sh
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
# shellcheck source=/dev/null
DEV_SH_LIB=1 . "$HERE/dev.sh"

TMP="$(mktemp -d)"
cleanup_test() {
  [ -n "${OLD_PID:-}" ] && kill "$OLD_PID" 2>/dev/null
  [ -n "${SPAWNED_PID:-}" ] && kill "$SPAWNED_PID" 2>/dev/null
  [ -n "${LISTENER:-}" ] && kill "$LISTENER" 2>/dev/null
  rm -rf "$TMP"
  return 0
}
trap cleanup_test EXIT

fail=0
ok() { printf '  ok   %s\n' "$1"; }
bad() {
  printf '  FAIL %s\n' "$1"
  fail=1
}

echo "port_probe:"
FREE=54999
if port_probe "$FREE"; then bad "a free port reads as up"; else ok "a free port reads as down"; fi
if command -v python3 >/dev/null 2>&1; then
  python3 -c "import socket,time;s=socket.socket();s.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);s.bind(('127.0.0.1',$FREE));s.listen(1);open('$TMP/ready','w').close();time.sleep(5)" &
  LISTENER=$!
  for _ in $(seq 1 40); do [ -f "$TMP/ready" ] && break; sleep 0.05; done
  if port_probe "$FREE"; then ok "a bound port reads as up"; else bad "misses a bound port"; fi
  kill "$LISTENER" 2>/dev/null
  LISTENER=""
else
  echo "  skip (no python3) — bound-port probe"
fi

echo "restart_node honesty:"
# Layout: a pinned NODE_BIN (staged copy the app dials) + a NODE_SRC (cargo's
# build output). The OLD running node is a sleeper so node_pids finds a live pid
# to restart; the rebuilt NODE_SRC is a crash-on-boot node, so the respawn must
# be reported as ✗ with the log reason — never a false ✓.
NODE_SRC="$TMP/node-src"
NODE_BIN="$TMP/staged/ducktape-node"
mkdir -p "$TMP/staged" "$TMP/wsdir"
CFG="$TMP/wsdir/node.toml"
echo "id=0" >"$CFG"

# staged (running) node = a long sleeper
cat >"$NODE_BIN" <<'EOF'
#!/usr/bin/env bash
sleep 30
EOF
chmod +x "$NODE_BIN"
"$NODE_BIN" --config "$CFG" &
OLD_PID=$!
for _ in $(seq 1 40); do node_pids | grep -q . && break; sleep 0.05; done

# cargo stub: "builds" by mutating NODE_SRC so the hash-gate fires
CARGO="$TMP/cargo-stub"
cat >"$CARGO" <<EOF
#!/usr/bin/env bash
printf '%s' "\$RANDOM" >>"$NODE_SRC"
exit 0
EOF
chmod +x "$CARGO"

# NODE_SRC = a crash-on-boot node
cat >"$NODE_SRC" <<'EOF'
#!/usr/bin/env bash
echo "FATAL bind 127.0.0.1:8844: address already in use" >&2
exit 1
EOF
chmod +x "$NODE_SRC"

OUT=$(restart_node 2>&1)
printf '%s\n' "$OUT" | sed 's/^/    │ /'
printf '%s\n' "$OUT" | grep -q '✗ rebuilt node exited on start' && ok "reports ✗ for a dead respawn" || bad "did not report the dead respawn"
printf '%s\n' "$OUT" | grep -q '✓ node back' && bad "falsely reported ✓ over a corpse" || ok "did not falsely claim ✓"
printf '%s\n' "$OUT" | grep -q 'address already in use' && ok "tailed the real reason from daemon.log" || bad "did not tail the log reason"

echo "restart_node hash-gate:"
# No source change → cargo stub that does NOT mutate NODE_SRC → skip the bounce.
"$NODE_BIN" --config "$CFG" &
OLD_PID=$!
sleep 0.2
CARGO="true" # a build that changes nothing
OUT=$(restart_node 2>&1)
printf '%s\n' "$OUT" | sed 's/^/    │ /'
printf '%s\n' "$OUT" | grep -q 'unchanged — skipping restart' && ok "skips the bounce when the binary is unchanged" || bad "did not skip an unchanged rebuild"
kill "$OLD_PID" 2>/dev/null
OLD_PID=""

echo
if [ "$fail" = 0 ]; then
  echo "ALL PASS"
else
  echo "FAILURES"
fi
exit "$fail"
