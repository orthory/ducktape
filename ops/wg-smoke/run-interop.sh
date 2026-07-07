#!/bin/bash
# backend-interop container smoke — the ADR phase-2 wire-compat gate
# (docs/adr/2026-07-07-userspace-overlay-net.mdx): the userspace (TUN-less)
# overlay backend and today's TUN backend on ONE WireGuard network, proving
# the Noise handshake, wire format, and cryptokey routing match across
# backends.
#
# Two rootless-podman containers running the same probe binary
# (`cargo build -p overlay-net --example wg_interop`):
#
#   tun    — today's production backend (DefguardWireGuardEffect, BoringTun
#            over a TUN device, kernel TCP/IP). Gets CAP_NET_ADMIN and
#            /dev/net/tun, runs passive (no peer endpoint: it must learn the
#            socket node's endpoint from the wire — WireGuard roaming).
#   socket — the userspace backend (UserspaceWireGuardEffect + smoltcp),
#            with --cap-drop ALL and NO devices: the stock-desktop privilege
#            posture the ADR exists to win.
#
# assertions:
#   A. socket → tun: TCP echo + UDP echo (socket side initiates the
#      handshake; smoltcp TCP against kernel TCP over the real wire).
#   B. tun → socket: TCP echo + UDP echo (kernel-originated connections
#      terminate in the smoltcp listener/socket).
#   C. the socket container really is unprivileged (no /dev/net/tun).
set -uo pipefail

NET=dtiop
IP_TUN=172.32.0.10
IP_SOCKET=172.32.0.11
# the loopback suite's fixture /48, one member /128 per side.
ULA_TUN=fda2:8ad3:eaee::1
ULA_SOCKET=fda2:8ad3:eaee::2
WG_PORT=51820
SCRATCH="$(cd "$(dirname "$0")" && pwd)"
LOG="$SCRATCH/interop.log"
BIN="${BIN:-$(cd "$SCRATCH/../.." && pwd)/target/debug/examples/wg_interop}"
# arch + openresolv + iptables — shared with the invite-orchestration smoke;
# baked here if absent (the probe itself only needs iproute2, in arch base).
IMG=localhost/dtinv-base

fail() { echo "INTEROP FAIL: $*" | tee -a "$LOG"; exit 1; }
note() { echo "--- $*" | tee -a "$LOG"; }

cleanup() {
  podman rm -f dtiop-tun dtiop-socket >/dev/null 2>&1
  podman network rm $NET >/dev/null 2>&1
}
cleanup
: > "$LOG"
[ -x "$BIN" ] || fail "no probe binary at $BIN (cargo build -p overlay-net --example wg_interop)"

if ! podman image exists "$IMG"; then
  note "baking base image (arch + openresolv + iptables)"
  podman rm -f dtiop-prep >/dev/null 2>&1
  podman run --name dtiop-prep docker.io/library/archlinux:latest \
    pacman -Sy --noconfirm openresolv iptables >/dev/null 2>&1 || fail "image prep (pacman)"
  podman commit dtiop-prep "$IMG" >/dev/null || fail "image commit"
  podman rm dtiop-prep >/dev/null
fi

podman network create --subnet 172.32.0.0/24 $NET >/dev/null || fail "network create"

note "keys: deterministic probe fixtures"
PRIV_TUN=$("$BIN" keygen 11 | sed -n 's/^PRIV //p')
PUB_TUN=$("$BIN" keygen 11 | sed -n 's/^PUB //p')
PRIV_SOCKET=$("$BIN" keygen 22 | sed -n 's/^PRIV //p')
PUB_SOCKET=$("$BIN" keygen 22 | sed -n 's/^PUB //p')
[ -n "$PRIV_TUN" ] && [ -n "$PUB_SOCKET" ] || fail "keygen"

note "starting the TUN-backend container (CAP_NET_ADMIN + /dev/net/tun, passive peer)"
podman run -d --name dtiop-tun --network $NET --ip $IP_TUN \
  --cap-add NET_ADMIN --device /dev/net/tun \
  -v "$BIN":/usr/local/bin/wg-interop:ro \
  $IMG sh -c "mkdir -p /run/wireguard && exec wg-interop serve --mode tun \
    --priv $PRIV_TUN --ula $ULA_TUN --wg-port $WG_PORT \
    --peer-pub $PUB_SOCKET --peer-ula $ULA_SOCKET" >/dev/null || fail "start tun container"

note "starting the socket-backend container (--cap-drop ALL, no devices, dialing)"
podman run -d --name dtiop-socket --network $NET --ip $IP_SOCKET \
  --cap-drop ALL \
  -v "$BIN":/usr/local/bin/wg-interop:ro \
  $IMG wg-interop serve --mode socket \
    --priv "$PRIV_SOCKET" --ula $ULA_SOCKET --wg-port $WG_PORT \
    --peer-pub "$PUB_TUN" --peer-ula $ULA_TUN \
    --peer-endpoint "$IP_TUN:$WG_PORT" --dial >/dev/null || fail "start socket container"

wait_marker() { # container marker timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if podman logs "$1" 2>&1 | grep -qF "$2"; then return 0; fi
    sleep 2
  done
  { echo "== logs $1 =="; podman logs "$1" 2>&1; } >> "$LOG"
  fail "$1 never printed: $2"
}

wait_marker dtiop-tun "INTEROP: serving" 60
wait_marker dtiop-socket "INTEROP: serving" 60
note "both backends up on one network"

note "leg A: socket → tun (handshake initiated by the unprivileged side)"
wait_marker dtiop-socket "INTEROP: tcp echo PASS" 90
wait_marker dtiop-socket "INTEROP: udp echo PASS" 90
note "leg A PASS: smoltcp TCP + UDP echo against the TUN backend"

note "leg B: tun → socket (kernel-originated connections into smoltcp)"
podman exec dtiop-tun wg-interop client tcp "[$ULA_SOCKET]:7000" | tee -a "$LOG" \
  | grep -q "CLIENT tcp PASS" || fail "tun→socket tcp echo"
podman exec dtiop-tun wg-interop client udp "[$ULA_SOCKET]:7002" | tee -a "$LOG" \
  | grep -q "CLIENT udp PASS" || fail "tun→socket udp echo"
note "leg B PASS: kernel TCP + UDP echo against the userspace backend"

note "leg C: the socket container is genuinely unprivileged"
podman exec dtiop-socket sh -c 'test ! -e /dev/net/tun' || fail "socket container has a TUN device"
note "leg C PASS: no /dev/net/tun, all caps dropped"

{ echo "== logs dtiop-tun =="; podman logs dtiop-tun 2>&1;
  echo "== logs dtiop-socket =="; podman logs dtiop-socket 2>&1; } >> "$LOG"
cleanup
echo "INTEROP PASS: userspace ↔ TUN wire compatibility proven (log: $LOG)"
