#!/bin/bash
# MIXED-MODE mesh-over-tunnel container smoke, including the COLD-RESTART
# leg — the overlay-net ADR's phase-3 standing gate
# (docs/adr/2026-07-07-userspace-overlay-net.mdx).
#
# Two real-WireGuard ducktape nodes (dev-seed shape) on a rootless-podman
# network, both with `advertised = "overlay"` — ONE PER BACKEND:
#
#   node0 — `wireguard_effect = "tun"` (BoringTun over a TUN device, kernel
#           TCP/IP): CAP_NET_ADMIN + /dev/net/tun, today's server posture.
#   node1 — `wireguard_effect = "socket"` (the TUN-less userspace backend):
#           NO /dev/net/tun — private networking with no interface, no
#           routes, no host mutation. (It keeps CAP_NET_ADMIN solely so the
#           harness can cut its own underlay with iptables; the backend
#           itself uses none of it, which the no-dt-interface evidence leg
#           asserts.)
#
# Assertions:
#   1. tunnels apply on both nodes (a dt-* interface on node0; the
#      in-process backend on node1 — and NO dt-* interface there),
#   2. consensus finalizes (heights advance) across the mixed pair,
#   3. cut the underlay TCP path (iptables, both directions, -p tcp only —
#      WG UDP untouched) and heights must KEEP advancing: mesh traffic
#      re-dials the peers' overlay ULAs and rides the tunnel. node0→node1
#      terminates in node1's VIRTUAL stack (the mesh listener's lazy leg);
#      node1→node0 is a virtual dial into node0's kernel-routed TUN.
#   4. THE COLD-RESTART PROOF: stop BOTH containers, restart them with the
#      underlay TCP blocked FROM BOOT (fresh netns dropped the phase-3
#      rules; a marker file re-applies them before the node starts). With
#      zero live TCP paths and tunnels gone, only the persisted mesh can
#      bring the network back: both nodes must restore tunnels from disk —
#      node1's into the userspace backend — node0 must dial node1's
#      persisted control ULA, live assembly must re-apply, and heights must
#      advance past their pre-restart values.
set -uo pipefail

NET=dtwg-smoke
SUBNET=172.30.0.0/24
IP0=172.30.0.10
IP1=172.30.0.11
SCRATCH="$(cd "$(dirname "$0")" && pwd)"
LOG="$SCRATCH/smoke.log"
BIN="${BIN:-$(cd "$SCRATCH/../.." && pwd)/target/debug/ducktape}"
# arch + openresolv + iptables — the same base the interop smoke bakes (a
# host-built binary needs the host's glibc; the debian rust image's is too
# old). baked here if absent.
IMG=localhost/dtinv-base

fail() { echo "SMOKE FAIL: $*" | tee -a "$LOG"; exit 1; }
note() { echo "--- $*" | tee -a "$LOG"; }

cleanup() {
  podman rm -f dtwg-node0 dtwg-node1 >/dev/null 2>&1
  podman network rm $NET >/dev/null 2>&1
}
cleanup
: > "$LOG"
[ -x "$BIN" ] || fail "no node binary at $BIN (cargo build -p node-bin)"

if ! podman image exists "$IMG"; then
  note "baking base image (arch + openresolv + iptables)"
  podman rm -f dtwg-prep >/dev/null 2>&1
  podman run --name dtwg-prep docker.io/library/archlinux:latest \
    pacman -Sy --noconfirm openresolv iptables >/dev/null 2>&1 || fail "image prep (pacman)"
  podman commit dtwg-prep "$IMG" >/dev/null || fail "image commit"
  podman rm dtwg-prep >/dev/null
fi

podman network create --subnet $SUBNET $NET >/dev/null || fail "network create"

mkdir -p "$SCRATCH/node0" "$SCRATCH/node1"
rm -rf "$SCRATCH/node0/storage" "$SCRATCH/node1/storage"
# phase-4 markers from a previous run would block the underlay from the
# FIRST boot — which is the unfixable first-join brick, not this test.
rm -f "$SCRATCH/node0/block-underlay" "$SCRATCH/node1/block-underlay"

cat > "$SCRATCH/node0/node.toml" <<TOML
id = 0
namespace = "wgsmoke"
peer_seeds = [0, 1]
listen = "[::]:41000"
advertised = "overlay"
wireguard_listen = "$IP0:51820"
wireguard_effect = "tun"
rpc_listen = "127.0.0.1:41100"
storage_dir = "/data/storage"
TOML

cat > "$SCRATCH/node1/node.toml" <<TOML
id = 1
namespace = "wgsmoke"
peer_seeds = [0, 1]
bootstrapper_addr = "$IP0:41000"
listen = "[::]:41000"
advertised = "overlay"
wireguard_listen = "$IP1:51820"
wireguard_effect = "socket"
rpc_listen = "127.0.0.1:41100"
storage_dir = "/data/storage"
TOML

# the entrypoint re-runs on every `podman start`: when /data/block-underlay
# exists (written before the cold restart), the peer's TCP underlay is
# REJECTed before the node comes up — a restart into a world with no
# usable TCP ingress, which only the persisted mesh can escape.
ENTRY='
  if [ -f /data/block-underlay ]; then
    PEER=$(cat /data/block-underlay) &&
    iptables -A OUTPUT -d "$PEER" -p tcp -j REJECT &&
    iptables -A INPUT -s "$PEER" -p tcp -j REJECT;
  fi &&
  mkdir -p /run/wireguard &&
  exec ducktape node --config /data/node.toml'

# node0: the TUN backend — privileged, device-backed.
podman run -d --name dtwg-node0 --network $NET --ip "$IP0" \
  --cap-add NET_ADMIN --device /dev/net/tun \
  -v "$BIN":/usr/local/bin/ducktape:ro -v "$SCRATCH/node0":/data \
  $IMG bash -c "$ENTRY" >/dev/null || fail "start node0"
# node1: the userspace socket backend — NO tun device to be had.
podman run -d --name dtwg-node1 --network $NET --ip "$IP1" \
  --cap-add NET_ADMIN \
  -v "$BIN":/usr/local/bin/ducktape:ro -v "$SCRATCH/node1":/data \
  $IMG bash -c "$ENTRY" >/dev/null || fail "start node1"

wait_marker() { # container marker timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if podman logs "$1" 2>&1 | grep -q "$2"; then return 0; fi
    sleep 2
  done
  echo "== logs $1 ==" >> "$LOG"; podman logs "$1" >> "$LOG" 2>&1
  fail "$1 never printed: $2"
}

note "waiting for tunnels on both nodes (1 peer each — a 0-peer apply is a FAILED epoch)"
wait_marker dtwg-node0 "tunnels applied on dt-.*(1 peer" 480
wait_marker dtwg-node1 "tunnels applied on dt-.*(1 peer(s); userspace socket backend" 480
note "tunnels applied with peers (node0 tun, node1 socket)"

height() { # container
  podman exec "$1" bash -c \
    'exec 3<>/dev/tcp/127.0.0.1/41100 && echo "{\"cmd\":\"status\"}" >&3 && head -1 <&3' \
    2>/dev/null | sed -n 's/.*"height":\([0-9]*\).*/\1/p'
}

wait_height_past() { # container floor timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    local h; h=$(height "$1")
    if [ -n "$h" ] && [ "$h" -gt "$2" ]; then echo "$h"; return 0; fi
    sleep 3
  done
  return 1
}

note "baseline liveness (heights advance pre-cut, across the mixed pair)"
H0=$(wait_height_past dtwg-node0 2 120) || fail "node0 height stuck pre-cut"
H1=$(wait_height_past dtwg-node1 2 120) || fail "node1 height stuck pre-cut"
note "pre-cut heights: node0=$H0 node1=$H1"

note "cutting underlay TCP both directions (WG UDP stays open)"
podman exec dtwg-node0 bash -c \
  "iptables -A OUTPUT -d $IP1 -p tcp -j REJECT && iptables -A INPUT -s $IP1 -p tcp -j REJECT" \
  || fail "iptables node0"
podman exec dtwg-node1 bash -c \
  "iptables -A OUTPUT -d $IP0 -p tcp -j REJECT && iptables -A INPUT -s $IP0 -p tcp -j REJECT" \
  || fail "iptables node1"

note "waiting for mesh to re-dial over the tunnel and consensus to resume"
HA=$(wait_height_past dtwg-node0 $(( H0 + 3 )) 180) || fail "node0 did not advance after the underlay cut"
HB=$(wait_height_past dtwg-node1 $(( H1 + 3 )) 180) || fail "node1 did not advance after the underlay cut"
note "post-cut heights: node0=$HA node1=$HB — consensus rides the mixed-mode tunnel"

note "evidence: node0 carries the overlay on a real interface"
podman exec dtwg-node0 ss -6 -t state established | tee -a "$LOG"
podman exec dtwg-node0 ip -6 addr show | grep -A1 "dt-" | tee -a "$LOG"
note "evidence: node1 carries it with NO interface at all (userspace backend)"
podman exec dtwg-node1 sh -c 'test ! -e /dev/net/tun' || fail "node1 has a TUN device"
podman exec dtwg-node1 sh -c '! ip link show | grep -q "dt-"' || fail "node1 grew a dt- interface"
echo "PHASE 1-3 PASS: mesh traffic flows tun<->socket over WireGuard after the underlay cut"

# ---- phase 4: whole-network cold restart with the underlay blocked from boot ----

note "cold restart: stopping BOTH nodes (tunnels die with the processes)"
podman stop dtwg-node0 dtwg-node1 >/dev/null || fail "stop"
echo "$IP1" > "$SCRATCH/node0/block-underlay"
echo "$IP0" > "$SCRATCH/node1/block-underlay"

[ -f "$SCRATCH/node0/storage/mesh-state.json" ] || fail "node0 never persisted its mesh"
[ -f "$SCRATCH/node1/storage/mesh-state.json" ] || fail "node1 never persisted its mesh"
note "persisted mesh state present on both nodes"

podman start dtwg-node0 dtwg-node1 >/dev/null || fail "restart"

wait_marker dtwg-node0 "persisted mesh (epoch .*) restored on dt-" 240
wait_marker dtwg-node1 "persisted mesh (epoch .*) restored on dt-" 240
note "both nodes restored tunnels from disk with zero TCP paths (node1 into the socket backend)"

# node1 has a configured hint for node0 (config wins — no seed); node0 has
# no hint for node1, so its dial path to node1 IS the persisted ULA seed.
wait_marker dtwg-node0 "1 mesh dial seed(s) from the persisted mesh" 60
note "node0 seeded its dialer from the persisted mesh"

# podman logs span both lives: the live re-assembly after restart is the
# SECOND "tunnels applied" on each node.
wait_marker_count() { # container marker count timeout
  local deadline=$(( $(date +%s) + $4 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ "$(podman logs "$1" 2>&1 | grep -cF "$2")" -ge "$3" ]; then return 0; fi
    sleep 3
  done
  echo "== logs $1 ==" >> "$LOG"; podman logs "$1" >> "$LOG" 2>&1
  fail "$1: fewer than $3 of: $2"
}
note "waiting for live assembly to replace the restored mesh"
wait_marker_count dtwg-node0 "tunnels applied on dt-" 2 300
wait_marker_count dtwg-node1 "tunnels applied on dt-" 2 300

note "post-restart liveness (heights must pass their pre-restart values)"
HC=$(wait_height_past dtwg-node0 "$HA" 300) || fail "node0 height stuck after cold restart"
HD=$(wait_height_past dtwg-node1 "$HB" 300) || fail "node1 height stuck after cold restart"
note "post-restart heights: node0=$HC node1=$HD (pre-restart: $HA/$HB)"

note "evidence: the underlay really is blocked and the mesh rides the overlay"
podman exec dtwg-node0 iptables -L OUTPUT -n | tee -a "$LOG"
podman exec dtwg-node0 ss -6 -t state established | tee -a "$LOG"

cleanup
echo "SMOKE PASS: mixed-mode (tun<->socket) cold restart healed from the persisted mesh with no TCP ingress"
