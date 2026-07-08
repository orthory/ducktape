#!/bin/bash
# overlay bulk-throughput bench — the ADR phase-4 perf gate
# (docs/adr/2026-07-07-userspace-overlay-net.mdx): raw bulk TCP over one
# WireGuard tunnel, kernel TCP over TUN vs in-process smoltcp, measured with
# the probe binary's bulk leg (`wg_interop serve --bulk`). deliberately NOT
# the statesync plane: no DataPlane, no bulk token bucket — this measures
# the STACK, which is the number that gates any non-desktop `socket`
# default. numbers are rig-relative (both containers share this host's
# CPU); the tun-vs-socket RATIO on the same rig is the result.
#
# four legs, one fresh container pair each (sender-mode → receiver-mode):
#   tun    → tun      today's server posture, the baseline
#   socket → socket   the all-userspace worst case (smoltcp both ends)
#   tun    → socket   desktop reality: a tun server pushing state at a
#                     socket desktop (statesync's joiner direction)
#   socket → tun      the reverse, for the full matrix
#
# each leg pushes BULK_BYTES (default 256 MiB) at the receiver's sink; the
# receiver's first-byte→EOF rate is the leg's number (the push side's rate
# is logged as a cross-check). pass = every leg delivers every byte; the
# table is the documented artifact.
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
# unoptimized ChaCha20 + smoltcp would understate both stacks.
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

# container privileges per backend mode: tun needs the device, socket runs
# with every cap dropped — the same postures the interop smoke pins.
run_probe() { # name ip mode extra-args...
  local name=$1 ip=$2 mode=$3; shift 3
  local caps
  if [ "$mode" = tun ]; then
    caps="--cap-add NET_ADMIN --device /dev/net/tun"
  else
    caps="--cap-drop ALL"
  fi
  # shellcheck disable=SC2086 — caps is deliberately word-split.
  podman run -d --name "$name" --network $NET --ip "$ip" $caps \
    -v "$BIN":/usr/local/bin/wg-interop:ro \
    $IMG sh -c "mkdir -p /run/wireguard && exec wg-interop $*" >/dev/null \
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

RESULTS=()
bench_leg() { # tx-mode rx-mode
  local tx_mode=$1 rx_mode=$2
  note "leg $tx_mode → $rx_mode: pushing $BULK_BYTES bytes"
  podman rm -f dtbench-rx dtbench-tx >/dev/null 2>&1
  # both sides get the peer's underlay endpoint so the handshake never
  # depends on which mode can initiate — the bench measures bulk, not rendezvous.
  run_probe dtbench-rx $IP_RX "$rx_mode" \
    serve --mode "$rx_mode" --priv "$PRIV_RX" --ula $ULA_RX --wg-port $WG_PORT \
    --peer-pub "$PUB_TX" --peer-ula $ULA_TX --peer-endpoint "$IP_TX:$WG_PORT"
  run_probe dtbench-tx $IP_TX "$tx_mode" \
    serve --mode "$tx_mode" --priv "$PRIV_TX" --ula $ULA_TX --wg-port $WG_PORT \
    --peer-pub "$PUB_RX" --peer-ula $ULA_RX --peer-endpoint "$IP_RX:$WG_PORT" \
    --bulk "$BULK_BYTES"
  # generous: the tun backend's bulk path runs single-digit Mbit/s (BoringTun
  # pipeline reordering — see the ADR's phase-4 results), so 256 MiB can
  # legitimately take minutes.
  local sink push
  sink=$(wait_line dtbench-rx "INTEROP: bulk sink" 900) \
    || fail "$tx_mode→$rx_mode: sink never finished (log: $LOG)"
  push=$(wait_line dtbench-tx "INTEROP: bulk push" 60) \
    || fail "$tx_mode→$rx_mode: push never finished (log: $LOG)"
  echo "$sink" | grep -q " $BULK_BYTES bytes " \
    || fail "$tx_mode→$rx_mode sink got a short read: $sink (pushed: $push)"
  local rate
  rate=$(echo "$sink" | sed -n 's/.*= \(.*MB\/s\)$/\1/p')
  note "leg $tx_mode → $rx_mode: sink $rate (push side: ${push#INTEROP: bulk push })"
  RESULTS+=("$(printf '%-6s → %-6s  %s' "$tx_mode" "$rx_mode" "$rate")")
}

bench_leg tun tun
bench_leg socket socket
bench_leg tun socket
bench_leg socket tun

{ echo "== rig =="; uname -r; nproc; } >> "$LOG"
cleanup

echo ""
echo "BULK THROUGHPUT (${BULK_BYTES} bytes/leg, receiver first-byte→EOF, $(nproc) cpus, $(uname -r))"
echo "  sender → receiver  rate"
for r in "${RESULTS[@]}"; do echo "  $r"; done
echo ""
echo "BENCH PASS: all legs delivered every byte (log: $LOG)"
