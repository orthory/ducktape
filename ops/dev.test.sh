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
bun -e 'Bun.listen({hostname:"127.0.0.1",port:Number(process.argv[1]),socket:{data(){}}})' "$FREE" 2>"$TMP/listener.err" &
LISTENER=$!
sleep 0.2
if ! kill -0 "$LISTENER" 2>/dev/null; then
  ok "bound-port check skipped (sandbox denied localhost listen)"
elif port_probe "$FREE"; then
  ok "a bound port reads as up"
else
  bad "misses a bound port"
fi
kill "$LISTENER" 2>/dev/null || true
wait "$LISTENER" 2>/dev/null || true
LISTENER=""

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
DUCKTAPE_DEV_NODE_PIDS="$OLD_PID"
DUCKTAPE_DEV_NODE_CONFIG="$CFG"

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
unset DUCKTAPE_DEV_NODE_PIDS DUCKTAPE_DEV_NODE_CONFIG
wait "$OLD_PID" 2>/dev/null || true
OLD_PID=""

echo "restart_node hash-gate:"
# No source change → cargo stub that does NOT mutate NODE_SRC → skip the bounce.
cat >"$NODE_BIN" <<'EOF'
#!/usr/bin/env bash
sleep 30
EOF
chmod +x "$NODE_BIN"
"$NODE_BIN" --config "$CFG" &
OLD_PID=$!
sleep 0.2
CARGO="true" # a build that changes nothing
OUT=$(restart_node 2>&1)
printf '%s\n' "$OUT" | sed 's/^/    │ /'
printf '%s\n' "$OUT" | grep -q 'unchanged — skipping restart' && ok "skips the bounce when the binary is unchanged" || bad "did not skip an unchanged rebuild"
kill "$OLD_PID" 2>/dev/null
wait "$OLD_PID" 2>/dev/null || true
OLD_PID=""

echo "app dependency preflight:"
APPDIR="$TMP/app"
mkdir -p "$APPDIR"
printf '{}\n' >"$APPDIR/package.json"
printf 'lock\n' >"$APPDIR/bun.lock"
BUN_LOG="$TMP/bun.log"
BUN="$TMP/bun-stub"
cat >"$BUN" <<EOF
#!/usr/bin/env bash
printf '%s\n' "\$*" >>"$BUN_LOG"
mkdir -p node_modules/@byeongsu-hong/tauri-agent-plugin
EOF
chmod +x "$BUN"
ensure_app_deps "$APPDIR" || bad "dependency preflight failed on missing node_modules"
grep -q 'install --frozen-lockfile' "$BUN_LOG" && ok "installs with the lockfile frozen" || bad "did not run frozen bun install"
: >"$BUN_LOG"
ensure_app_deps "$APPDIR" || bad "dependency preflight failed on fresh node_modules"
[ ! -s "$BUN_LOG" ] && ok "skips install when node_modules is fresh" || bad "reinstalled fresh node_modules"
sleep 1
touch "$APPDIR/package.json"
ensure_app_deps "$APPDIR" || bad "dependency preflight failed on stale node_modules"
grep -q 'install --frozen-lockfile' "$BUN_LOG" && ok "refreshes stale node_modules" || bad "did not refresh stale node_modules"

echo "platform branch selection:"
DUCKTAPE_DEV_OS=Darwin
[ "$(dev_os)" = Darwin ] && ok "honors Darwin platform override" || bad "missed Darwin platform override"
DUCKTAPE_DEV_OS=Linux
[ "$(dev_os)" = Linux ] && ok "honors Linux platform override" || bad "missed Linux platform override"
unset DUCKTAPE_DEV_OS

if grep -q 'DUCKTAPE_DISABLE_HEARTBEAT' "$HERE/dev.sh"; then
  bad "dev disables heartbeats and can strand writes at height zero"
else
  ok "keeps heartbeats enabled so dev writes finalize"
fi

echo "macOS cargo runner contract:"
RUNNER_LOG="$TMP/runner.log"
RUNNER_STUB="$TMP/build-with-stub"
cat >"$RUNNER_STUB" <<'EOF'
#!/usr/bin/env bash
printf 'runner=%s\n' "${CARGO_TARGET_AARCH64_APPLE_DARWIN_RUNNER:-}" >"$RUNNER_LOG"
printf 'args=%s\n' "$*" >>"$RUNNER_LOG"
EOF
chmod +x "$RUNNER_STUB"
export RUNNER_LOG
BUILD_WITH="$RUNNER_STUB" CARGO="cargo-test" "$HERE/dev-macos-cargo.sh" run --target aarch64-apple-darwin
grep -q "runner=$HERE/dev-macos-runner.sh" "$RUNNER_LOG" \
  && ok "points explicit-target Cargo at the bundle runner" \
  || bad "did not export the macOS target runner"
grep -q 'args=cargo-test run --target aarch64-apple-darwin' "$RUNNER_LOG" \
  && ok "preserves the Cargo-compatible runner arguments" \
  || bad "changed the Cargo runner argument contract"
grep -q -- '--features dev-cef' "$HERE/dev.sh" \
  && grep -q -- '--no-default-features' "$HERE/dev.sh" \
  && ok "avoids the incomplete CEF CLI dev bundler with the dependency-equivalent feature" \
  || bad "macOS dev can still enter the incomplete CEF CLI bundler"

echo "macOS bundle staging:"
ROOT="$TMP/root"
mkdir -p "$ROOT/ops" "$ROOT/target/debug" "$ROOT/skeleton/Contents/MacOS" \
  "$ROOT/skeleton/Contents/Frameworks/Chromium Embedded Framework.framework/Resources"
cp "$HERE/check-macos-cef-bundle.sh" "$ROOT/ops/check-macos-cef-bundle.sh"
printf 'framework\n' >"$ROOT/skeleton/Contents/Frameworks/Chromium Embedded Framework.framework/Chromium Embedded Framework"
printf 'icu\n' >"$ROOT/skeleton/Contents/Frameworks/Chromium Embedded Framework.framework/Resources/icudtl.dat"
printf 'old\n' >"$ROOT/skeleton/Contents/MacOS/ducktape-desktop"
for helper in $(macos_helper_names | sed 's/ /_/g'); do
  helper_name=$(printf '%s' "$helper" | sed 's/_/ /g')
  helper_exe="$ROOT/skeleton/Contents/Frameworks/$helper_name.app/Contents/MacOS/$helper_name"
  mkdir -p "${helper_exe%/*}"
  printf 'old\n' >"$helper_exe"
  chmod +x "$helper_exe"
done
chmod +x "$ROOT/skeleton/Contents/MacOS/ducktape-desktop"
DEBUG_EXE="$ROOT/target/debug/ducktape-desktop"
printf 'new-debug-binary\n' >"$DEBUG_EXE"
chmod +x "$DEBUG_EXE"
mkdir -p "$ROOT/invalid"
MACOS_BUNDLE_SOURCE="$ROOT/skeleton"
MACOS_SYSTEM_APP="$ROOT/no-system-app"
MACOS_USER_APP="$ROOT/no-user-app"
MACOS_DEBUG_APP="$ROOT/target/debug/Ducktape.app"
STAGED=$(stage_macos_debug_bundle "$DEBUG_EXE")
[ "$STAGED" = "$MACOS_DEBUG_APP" ] && ok "stages under target/debug" || bad "staged bundle in the wrong location"
cmp -s "$DEBUG_EXE" "$STAGED/Contents/MacOS/ducktape-desktop" && ok "replaces main executable" || bad "main executable was not replaced"
helpers_ok=1
while IFS= read -r helper; do
  cmp -s "$DEBUG_EXE" "$STAGED/Contents/Frameworks/$helper.app/Contents/MacOS/$helper" || helpers_ok=0
done < <(macos_helper_names)
[ "$helpers_ok" = 1 ] && ok "replaces all five helper executables" || bad "one or more helper executables were stale"
MACOS_BUNDLE_SOURCE="$ROOT/invalid"
if macos_bundle_source >/dev/null; then
  bad "accepted a bundle directory without CEF payloads"
else
  ok "rejects incomplete bundle directories before launch"
fi
rm -rf "$ROOT"
unset ROOT MACOS_BUNDLE_SOURCE MACOS_SYSTEM_APP MACOS_USER_APP MACOS_DEBUG_APP

echo "stale macOS app cleanup scope:"
sleep 30 &
STALE_APP_PID=$!
sleep 30 &
OTHER_APP_PID=$!
DUCKTAPE_DEV_APP_PIDS="$STALE_APP_PID"
stop_stale_macos_debug_app
wait "$STALE_APP_PID" 2>/dev/null || true
kill -0 "$STALE_APP_PID" 2>/dev/null \
  && bad "left the scoped stale debug app alive" \
  || ok "stops the scoped stale debug app"
kill -0 "$OTHER_APP_PID" 2>/dev/null \
  && ok "leaves unrelated app processes alone" \
  || bad "killed an unrelated app process"
kill "$OTHER_APP_PID" 2>/dev/null || true
wait "$OTHER_APP_PID" 2>/dev/null || true
unset DUCKTAPE_DEV_APP_PIDS

echo "cleanup scope:"
OWNED_FILE="$TMP/cfg"
STAMP_FILE="$TMP/stamp"
printf 'x' >"$OWNED_FILE"
printf 'x' >"$STAMP_FILE"
sleep 30 &
WATCH_PID=$!
sleep 30 &
OTHER_PID=$!
CFG_OVERRIDE="$OWNED_FILE"
cleanup
wait "$WATCH_PID" 2>/dev/null || true
sleep 0.2
kill -0 "$WATCH_PID" 2>/dev/null && bad "cleanup left owned watcher alive" || ok "cleanup stops the owned watcher"
kill -0 "$OTHER_PID" 2>/dev/null && ok "cleanup leaves unrelated processes alone" || bad "cleanup killed an unrelated process"
[ ! -e "$OWNED_FILE" ] && [ ! -e "$STAMP_FILE" ] && ok "cleanup removes owned temp files" || bad "cleanup left owned temp files"
kill "$OTHER_PID" 2>/dev/null || true
wait "$OTHER_PID" 2>/dev/null || true
WATCH_PID=""
OTHER_PID=""
CFG_OVERRIDE=""
STAMP_FILE=""

echo
if [ "$fail" = 0 ]; then
  echo "ALL PASS"
else
  echo "FAILURES"
fi
exit "$fail"
