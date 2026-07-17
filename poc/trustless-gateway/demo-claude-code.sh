#!/usr/bin/env bash
# Local Claude Code demo: the REAL `claude` CLI runs against our TEE-broker host
# using ONLY a scoped, temporary session token. Hermetic by default — a mock
# upstream stands in for Anthropic, so no real credentials and no ToS exposure;
# the credential-swap + token-custody path is still fully exercised.
#
# To point at REAL Anthropic instead (spends subscription; account-sharing
# exposure):
#   UPSTREAM_BASE=https://api.anthropic.com \
#   OAUTH_URL=https://console.anthropic.com/v1/oauth/token \
#   CREDS=$HOME/.claude/.credentials.json ./demo-claude-code.sh
set -euo pipefail
cd "$(dirname "$0")"

command -v claude >/dev/null || { echo "need the 'claude' CLI on PATH"; exit 1; }

MEAS=$(printf '11%.0s' $(seq 1 48))
HOST=http://127.0.0.1:9100
UPSTREAM_BASE=${UPSTREAM_BASE:-http://127.0.0.1:9101}
OAUTH_URL=${OAUTH_URL:-http://127.0.0.1:9101/oauth/token}
CLIENT_ID=${CLIENT_ID:-9d1c250a-e61b-44d9-88ed-5944d1962f5e}
REFRESH=${REFRESH:-ref-seed}

echo "== build =="
cargo build --quiet -p tcg-host -p tcg-client
H=target/debug/tcg-host
C=target/debug/tcg-client
cleanup() { kill "${MOCK:-}" "${HOSTP:-}" 2>/dev/null || true; }
trap cleanup EXIT

if [ "$UPSTREAM_BASE" = "http://127.0.0.1:9101" ]; then
    "$H" mock-upstream --listen 127.0.0.1:9101 &
    MOCK=$!
fi

echo "== boot host (mock attest; upstream=$UPSTREAM_BASE) =="
"$H" serve --listen 127.0.0.1:9100 --attest mock --measurement "$MEAS" \
    --anthropic-base "$UPSTREAM_BASE" --oauth-token-url "$OAUTH_URL" --oauth-client-id "$CLIENT_ID" &
HOSTP=$!
for _ in $(seq 1 50); do
    curl -sf "$HOST/attestation" >/dev/null 2>&1 && break
    sleep 0.1
done

echo "== seal credential to enclave =="
if [ -n "${CREDS:-}" ]; then
    "$C" seal --host "$HOST" --attest mock --measurement "$MEAS" --credentials "$CREDS"
else
    "$C" seal --host "$HOST" --attest mock --measurement "$MEAS" --refresh-token "$REFRESH"
fi

echo "== mint a temporary session token (attested handshake) =="
TOKEN=$("$C" token --host "$HOST" --attest mock --measurement "$MEAS" --sub claude-code)
echo "token: ${TOKEN:0:24}…"

echo "== run Claude Code through the enclave with ONLY that token =="
work=$(mktemp -d)
set +e
OUT=$(cd "$work" && \
    CLAUDE_CONFIG_DIR="$work/cfg" \
    ANTHROPIC_BASE_URL="$HOST" \
    ANTHROPIC_AUTH_TOKEN="$TOKEN" \
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC=1 \
    claude -p "Reply with exactly: TRUSTLESS-GATEWAY-OK" 2>&1)
rc=$?
set -e
echo "--- claude output ---"
echo "$OUT"
echo "---------------------"
rm -rf "$work"

if echo "$OUT" | grep -q "TRUSTLESS-GATEWAY-OK"; then
    echo "PASS ✅  Claude Code ran through the enclave broker with a temporary token"
else
    echo "FAIL ❌ (claude rc=$rc)"
    exit 1
fi
