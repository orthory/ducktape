#!/usr/bin/env bash
# Run all three APP-LAYER huddle tests against the live callbed — the layers
# above the raw transport (call-driver.ts), proving the real app can huddle:
#   L1  huddleRecipients unit           — roster -> fan-out set (real fn)
#   L2  join_huddle consensus propagation — a join op crosses to the other node's roster
#   L3  real call client in Chromium     — call-session.ts audio+video over the mesh
#
# Prereq: the callbed is up and published on the host —
#   docker compose -f ops/callbed/docker-compose.yml up -d --wait node0 node1
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
BUN="${BUN:-/home/eddy/.local/bin/bun}"
A="${1:-127.0.0.1:8080}" B="${2:-127.0.0.1:8081}"
fail=0

echo "==== L1: huddleRecipients unit ===================================="
( cd "$HERE/tests" && "$BUN" test recipients.test.ts ) || fail=1

echo "==== L2: join_huddle -> consensus roster propagation =============="
"$BUN" "$HERE/joinhuddle-rpc.ts" "$A" "$B" || fail=1

echo "==== L3: real call client (audio+video) in headless Chromium ======"
bash "$HERE/run-app-e2e.sh" "${A##*:}" "${B##*:}" || fail=1

echo "=================================================================="
[ $fail -eq 0 ] && echo "ALL APP-LAYER TESTS PASS ✓" || echo "SOME TESTS FAILED ✗"
exit $fail
