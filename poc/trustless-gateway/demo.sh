#!/usr/bin/env bash
# Hermetic end-to-end demo: --attest mock --upstream mock. No real credentials,
# no network. Boots mock-upstream + host, seals a credential, runs a proxied
# call, asserts the model reply came back through the enclave.
set -euo pipefail
cd "$(dirname "$0")"

MEAS=$(printf '11%.0s' $(seq 1 48))   # 48-byte measurement, all 0x11
HOST=http://127.0.0.1:9100

echo "== build =="
cargo build --quiet
HOST_BIN=target/debug/tcg-host
CLIENT_BIN=target/debug/tcg-client

cleanup() { kill "${MOCK_PID:-}" "${HOST_PID:-}" 2>/dev/null || true; }
trap cleanup EXIT

echo "== boot mock-upstream + host =="
"$HOST_BIN" mock-upstream --listen 127.0.0.1:9101 &
MOCK_PID=$!
"$HOST_BIN" serve --listen 127.0.0.1:9100 --attest mock --measurement "$MEAS" \
    --anthropic-base http://127.0.0.1:9101 \
    --oauth-token-url http://127.0.0.1:9101/oauth/token &
HOST_PID=$!

for _ in $(seq 1 50); do
    curl -sf "$HOST/attestation" >/dev/null 2>&1 && break
    sleep 0.1
done

echo "== seal (Credential Provider) =="
"$CLIENT_BIN" seal --host "$HOST" --attest mock --measurement "$MEAS" --refresh-token ref-seed

echo "== run (Computation Provider) =="
OUT=$("$CLIENT_BIN" run --host "$HOST" --sub demo --prompt "hi")
echo "$OUT"

if echo "$OUT" | grep -q "MOCK-REPLY-OK"; then
    echo "PASS ✅  — sandbox got a model reply through the enclave, never touching the credential"
else
    echo "FAIL ❌  — no reply through the proxy"
    exit 1
fi
