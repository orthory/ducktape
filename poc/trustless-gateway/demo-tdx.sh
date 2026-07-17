#!/usr/bin/env bash
# Full scenario on an Intel TDX box: REAL attestation (configfs-tsm quote +
# dcap-qvl verify). Run this INSIDE the TD guest.
#
# Prereqs inside the guest:
#   - kernel >= 6.7 with configfs-tsm; /sys/kernel/config/tsm/report must exist
#     (mount -t configfs none /sys/kernel/config  if it is not mounted)
#   - a working quote-generation path (QGS over vsock) so `outblob` is a signed
#     QUOTE, not just a TD report. Bare TDX guests without QGS return a report,
#     which dcap-qvl cannot verify. Azure/GCP confidential VMs wire this up.
#   - network egress to Intel PCS, or set PCCS_URL to your own PCCS.
#   - run as root (creating configfs report dirs) or with write access to it.
#
# Upstream defaults to the hermetic mock (zero ToS exposure). To validate the
# real OAuth constants against Anthropic instead:
#   UPSTREAM_BASE=https://api.anthropic.com \
#   OAUTH_URL=https://console.anthropic.com/v1/oauth/token \
#   CREDS=$HOME/.claude/.credentials.json ./demo-tdx.sh
set -euo pipefail
cd "$(dirname "$0")"

HOST=http://127.0.0.1:9100
UPSTREAM_BASE=${UPSTREAM_BASE:-http://127.0.0.1:9101}
OAUTH_URL=${OAUTH_URL:-http://127.0.0.1:9101/oauth/token}
CLIENT_ID=${CLIENT_ID:-9d1c250a-e61b-44d9-88ed-5944d1962f5e}
REFRESH=${REFRESH:-ref-seed}

echo "== build (host default, client --features tdx) =="
cargo build --quiet -p tcg-host
cargo build --quiet -p tcg-client --features tdx
H=target/debug/tcg-host
C=target/debug/tcg-client

cleanup() { kill "${MOCK:-}" "${HOSTP:-}" 2>/dev/null || true; }
trap cleanup EXIT

if [ "$UPSTREAM_BASE" = "http://127.0.0.1:9101" ]; then
    "$H" mock-upstream --listen 127.0.0.1:9101 &
    MOCK=$!
fi

echo "== boot host with REAL TDX attestation =="
"$H" serve --listen 127.0.0.1:9100 --attest tdx \
    --anthropic-base "$UPSTREAM_BASE" --oauth-token-url "$OAUTH_URL" --oauth-client-id "$CLIENT_ID" &
HOSTP=$!
for _ in $(seq 1 50); do
    curl -sf "$HOST/attestation" >/dev/null 2>&1 && break
    sleep 0.2
done

echo "== inspect: read MRTD from the real quote (TOFU pin) =="
MRTD=$("$C" inspect --host "$HOST" --attest tdx)   # stderr shows RTMRs + report_data
echo "pinned MRTD=$MRTD"

echo "== seal: dcap-qvl verifies the quote, then seals the refresh token =="
if [ -n "${CREDS:-}" ]; then
    "$C" seal --host "$HOST" --attest tdx --measurement "$MRTD" --credentials "$CREDS"
else
    "$C" seal --host "$HOST" --attest tdx --measurement "$MRTD" --refresh-token "$REFRESH"
fi

echo "== run: attested handshake -> scoped session token -> proxied call =="
OUT=$("$C" run --host "$HOST" --attest tdx --measurement "$MRTD" --sub demo --prompt "hi")
echo "$OUT"

if echo "$OUT" | grep -q TRUSTLESS-GATEWAY-OK; then
    echo "PASS ✅  real TDX attestation + mock upstream"
elif [ "$UPSTREAM_BASE" != "http://127.0.0.1:9101" ] && echo "$OUT" | grep -qi '"type"'; then
    echo "PASS ✅  real TDX attestation + REAL Anthropic upstream (OAuth constants valid)"
else
    echo "FAIL ❌"
    exit 1
fi
