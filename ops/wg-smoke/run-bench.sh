#!/bin/bash
# overlay bulk-throughput bench: raw bulk TCP over one WireGuard tunnel on
# the userspace backend (the node's only backend), measured with the probe
# binary's bulk leg (`wg_interop serve --bulk`). deliberately NOT the
# statesync plane: no DataPlane, no bulk token bucket — this measures the
# STACK. numbers are rig-relative (both containers share this host's CPU).
#
# (history: the four-leg tun/socket matrix here is what retired the TUN
# backend — see the phase-4 results table in
# docs/adr/2026-07-07-userspace-overlay-net.mdx.)
#
# one fresh container pair; the sender pushes BULK_BYTES (default 256 MiB)
# at the receiver's sink; the receiver's first-byte→EOF rate is the number
# (the push side's rate is logged as a cross-check). pass = every byte
# delivered.
set -uo pipefail

NET=dtbench
SUBNET=172.33.0.0/24
IP_RX=172.33.0.10
IP_TX=172.33.0.11
# the loopback suite's fixture /48, one member /128 per side.
ULA_RX=fda2:8ad3:eaee::1
ULA_TX=fda2:8ad3:eaee::2
WG_PORT=51820
BULK_BYTES="${BULK_BYTES:-268435456}" # 256 MiB
SCRATCH="$(cd "$(dirname "$0")" && pwd)"
LOG="$SCRATCH/bench.log"
# release, unlike the correctness smokes: a throughput number from an
# unoptimized ChaCha20 + smoltcp would understate the stack.
BIN="${BIN:-$(cd "$SCRATCH/../.." && pwd)/target/release/examples/wg_interop}"
# arch + openresolv + iptables — shared with the other wg-smoke rigs.
IMG=localhost/dtinv-base

fail() { echo "BENCH FAIL: $*" | tee -a "$LOG"; exit 1; }
note() { echo "--- $*" | tee -a "$LOG"; }

cleanup() {
  podman rm -f dtbench-rx dtbench-tx >/dev/null 2>&1
  podman network rm $NET >/dev/null 2>&1
}
cleanup
: > "$LOG"
[ -x "$BIN" ] || fail "no probe binary at $BIN (cargo build --release -p overlay-net --example wg_interop)"

if ! podman image exists "$IMG"; then
  note "baking base image (arch + openresolv + iptables)"
  podman rm -f dtbench-prep >/dev/null 2>&1
  podman run --name dtbench-prep docker.io/library/archlinux:latest \
    pacman -Sy --noconfirm openresolv iptables >/dev/null 2>&1 || fail "image prep (pacman)"
  podman commit dtbench-prep "$IMG" >/dev/null || fail "image commit"
  podman rm dtbench-prep >/dev/null
fi

podman network create --subnet $SUBNET $NET >/dev/null || fail "network create"

note "keys: deterministic probe fixtures"
PRIV_RX=$("$BIN" keygen 11 | sed -n 's/^PRIV //p')
PUB_RX=$("$BIN" keygen 11 | sed -n 's/^PUB //p')
PRIV_TX=$("$BIN" keygen 22 | sed -n 's/^PRIV //p')
PUB_TX=$("$BIN" keygen 22 | sed -n 's/^PUB //p')
[ -n "$PRIV_RX" ] && [ -n "$PUB_TX" ] || fail "keygen"

# the same fully-unprivileged posture the interop smoke pins.
run_probe() { # name ip extra-args...
  local name=$1 ip=$2; shift 2
  podman run -d --name "$name" --network $NET --ip "$ip" --cap-drop ALL \
    -v "$BIN":/usr/local/bin/wg-interop:ro \
    $IMG wg-interop "$@" >/dev/null \
    || fail "start $name"
}

# echoes the first matching line; returns 1 on timeout (dumping the container
# logs to $LOG). callers run under $() — fail() in here could not abort the
# script, so the abort belongs at the call site.
wait_line() { # container pattern timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    local line
    line=$(podman logs "$1" 2>&1 | grep -m1 "$2")
    if [ -n "$line" ]; then echo "$line"; return 0; fi
    sleep 2
  done
  { echo "== logs $1 (no '$2' within ${3}s) =="; podman logs "$1" 2>&1; } >> "$LOG"
  return 1
}

note "pushing $BULK_BYTES bytes tx → rx"
# both sides get the peer's underlay endpoint so the handshake never depends
# on which side initiates — the bench measures bulk, not rendezvous.
run_probe dtbench-rx $IP_RX \
  serve --priv "$PRIV_RX" --ula $ULA_RX --wg-port $WG_PORT \
  --peer-pub "$PUB_TX" --peer-ula $ULA_TX --peer-endpoint "$IP_TX:$WG_PORT"
run_probe dtbench-tx $IP_TX \
  serve --priv "$PRIV_TX" --ula $ULA_TX --wg-port $WG_PORT \
  --peer-pub "$PUB_RX" --peer-ula $ULA_RX --peer-endpoint "$IP_RX:$WG_PORT" \
  --bulk "$BULK_BYTES"
SINK=$(wait_line dtbench-rx "INTEROP: bulk sink" 300) \
  || fail "sink never finished (log: $LOG)"
PUSH=$(wait_line dtbench-tx "INTEROP: bulk push" 60) \
  || fail "push never finished (log: $LOG)"
echo "$SINK" | grep -q " $BULK_BYTES bytes " \
  || fail "sink got a short read: $SINK (pushed: $PUSH)"
RATE=$(echo "$SINK" | sed -n 's/.*= \(.*MB\/s\)$/\1/p')

{ echo "== rig =="; uname -r; nproc; } >> "$LOG"
cleanup

echo ""
echo "BULK THROUGHPUT (${BULK_BYTES} bytes, receiver first-byte→EOF, $(nproc) cpus, $(uname -r))"
echo "  socket → socket  $RATE (push side: ${PUSH#INTEROP: bulk push })"
echo ""
echo "BENCH PASS: every byte delivered (log: $LOG)"
