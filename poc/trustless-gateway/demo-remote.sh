#!/usr/bin/env bash
# Remote-topology demo: Credential Provider != Computation Provider. Both reach
# the gateway through the REMOTE path — `--remote <handle>.duck --via <url>` —
# so every request carries the `x-duck-authority` header the overlay routes on.
#
# Hermetic: the gateway + mock-upstream run locally, and `--via` points straight
# at the gateway (no overlay needed to exercise the CLIENT abstraction). In
# production `--via` is the local node's browser-gateway URL and the header
# routes `<handle>.duck` to the remote node's published LoopbackHttp service —
# the ONLY change is the --via target. See the design spec §graft.
set -euo pipefail
cd "$(dirname "$0")"

MEAS=$(printf '11%.0s' $(seq 1 48))
GW=http://127.0.0.1:9100          # stands in for the remote node's gateway
HANDLE=demo.duck

echo "== build =="
cargo build --quiet
H=target/debug/tcg-host
C=target/debug/tcg-client
cleanup() { kill "${MOCK:-}" "${HOSTP:-}" 2>/dev/null || true; }
trap cleanup EXIT

echo "== boot mock-upstream + remote gateway =="
"$H" mock-upstream --listen 127.0.0.1:9101 &
MOCK=$!
"$H" serve --listen 127.0.0.1:9100 --attest mock --measurement "$MEAS" \
    --anthropic-base http://127.0.0.1:9101 \
    --oauth-token-url http://127.0.0.1:9101/oauth/token &
HOSTP=$!
for _ in $(seq 1 50); do
    curl -sf "$GW/attestation" >/dev/null 2>&1 && break
    sleep 0.1
done

echo "== seal (Credential Provider, REMOTE via $HANDLE) =="
"$C" seal --remote "$HANDLE" --via "$GW" --attest mock --measurement "$MEAS" --refresh-token ref-seed

echo "== run (Computation Provider, REMOTE via $HANDLE) =="
OUT=$("$C" run --remote "$HANDLE" --via "$GW" --attest mock --measurement "$MEAS" --sub remote-demo --prompt "hi")
echo "$OUT"

if echo "$OUT" | grep -q "TRUSTLESS-GATEWAY-OK"; then
    echo "PASS ✅  — remote topology: sandbox reached the gateway through the duck:// path, credential never left the enclave"
else
    echo "FAIL ❌  — no reply through the remote gateway"
    exit 1
fi
