#!/bin/bash
# overlay wire-compat container smoke
# (docs/adr/2026-07-07-userspace-overlay-net.mdx): two fully unprivileged
# containers running the userspace overlay backend (the node's only backend)
# on ONE WireGuard network, proving the Noise handshake, wire format, and
# cryptokey routing over a real container network.
#
# Two rootless-podman containers running the same probe binary
# (`cargo build -p overlay-net --example wg_interop`), BOTH with
# --cap-drop ALL and NO devices: the stock privilege posture the backend
# exists to win. Each side dials the other, so both directions of every
# transport surface are exercised.
#
# assertions:
#   A. a → b and b → a: TCP echo + UDP echo through the overlay ULAs.
#   B. both containers really are unprivileged (no /dev/net/tun).
set -uo pipefail

NET=dtiop
IP_A=172.32.0.10
IP_B=172.32.0.11
# the loopback suite's fixture /48, one member /128 per side.
ULA_A=fda2:8ad3:eaee::1
ULA_B=fda2:8ad3:eaee::2
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
  podman rm -f dtiop-a dtiop-b >/dev/null 2>&1
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
PRIV_A=$("$BIN" keygen 11 | sed -n 's/^PRIV //p')
PUB_A=$("$BIN" keygen 11 | sed -n 's/^PUB //p')
PRIV_B=$("$BIN" keygen 22 | sed -n 's/^PRIV //p')
PUB_B=$("$BIN" keygen 22 | sed -n 's/^PUB //p')
[ -n "$PRIV_A" ] && [ -n "$PUB_B" ] || fail "keygen"

# both sides get the peer's underlay endpoint and dial: WireGuard tolerates
# simultaneous initiation, and the smoke wants both directions proven.
note "starting container a (--cap-drop ALL, no devices, dialing)"
podman run -d --name dtiop-a --network $NET --ip $IP_A \
  --cap-drop ALL \
  -v "$BIN":/usr/local/bin/wg-interop:ro \
  $IMG wg-interop serve \
    --priv "$PRIV_A" --ula $ULA_A --wg-port $WG_PORT \
    --peer-pub "$PUB_B" --peer-ula $ULA_B \
    --peer-endpoint "$IP_B:$WG_PORT" --dial >/dev/null || fail "start container a"

note "starting container b (--cap-drop ALL, no devices, dialing)"
podman run -d --name dtiop-b --network $NET --ip $IP_B \
  --cap-drop ALL \
  -v "$BIN":/usr/local/bin/wg-interop:ro \
  $IMG wg-interop serve \
    --priv "$PRIV_B" --ula $ULA_B --wg-port $WG_PORT \
    --peer-pub "$PUB_A" --peer-ula $ULA_A \
    --peer-endpoint "$IP_A:$WG_PORT" --dial >/dev/null || fail "start container b"

wait_marker() { # container marker timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if podman logs "$1" 2>&1 | grep -qF "$2"; then return 0; fi
    sleep 2
  done
  { echo "== logs $1 =="; podman logs "$1" 2>&1; } >> "$LOG"
  fail "$1 never printed: $2"
}

wait_marker dtiop-a "INTEROP: serving" 60
wait_marker dtiop-b "INTEROP: serving" 60
note "both sides up on one network"

note "leg A: echoes pass in BOTH directions (each side dials the other)"
wait_marker dtiop-a "INTEROP: tcp echo PASS" 90
wait_marker dtiop-a "INTEROP: udp echo PASS" 90
wait_marker dtiop-b "INTEROP: tcp echo PASS" 90
wait_marker dtiop-b "INTEROP: udp echo PASS" 90
note "leg A PASS: smoltcp TCP + UDP echo, both directions"

note "leg B: both containers are genuinely unprivileged"
podman exec dtiop-a sh -c 'test ! -e /dev/net/tun' || fail "container a has a TUN device"
podman exec dtiop-b sh -c 'test ! -e /dev/net/tun' || fail "container b has a TUN device"
note "leg B PASS: no /dev/net/tun, all caps dropped"

{ echo "== logs dtiop-a =="; podman logs dtiop-a 2>&1;
  echo "== logs dtiop-b =="; podman logs dtiop-b 2>&1; } >> "$LOG"
cleanup
echo "INTEROP PASS: userspace overlay wire compatibility proven (log: $LOG)"
