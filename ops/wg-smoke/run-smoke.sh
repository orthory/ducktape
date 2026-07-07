#!/bin/bash
# mesh-over-tunnel container smoke, including the COLD-RESTART leg.
#
# Two real-WireGuard ducktape nodes (dev-seed shape) on a docker user network,
# both with `advertised = "overlay"`. Assertions:
#   1. real tunnels apply on both nodes (dt-* interface),
#   2. consensus finalizes (heights advance),
#   3. cut the underlay TCP path (iptables, both directions, -p tcp only —
#      WG UDP untouched) and heights must KEEP advancing: mesh traffic
#      re-dials the peers' overlay ULAs and rides the tunnel.
#   4. THE COLD-RESTART PROOF: stop BOTH containers, restart them with the
#      underlay TCP blocked FROM BOOT (fresh netns dropped the phase-3
#      rules; a marker file re-applies them before the node starts). With
#      zero live TCP paths and tunnels gone, only the persisted mesh can
#      bring the network back: both nodes must restore tunnels from disk,
#      node0 must dial node1's persisted control ULA, live assembly must
#      re-apply, and heights must advance past their pre-restart values.
set -uo pipefail

NET=dtwg-smoke
IP0=172.30.0.10
IP1=172.30.0.11
SCRATCH="$(cd "$(dirname "$0")" && pwd)"
LOG="$SCRATCH/smoke.log"

fail() { echo "SMOKE FAIL: $*" | tee -a "$LOG"; exit 1; }
note() { echo "--- $*" | tee -a "$LOG"; }

cleanup() {
  docker rm -f dtwg-node0 dtwg-node1 >/dev/null 2>&1
  docker network rm $NET >/dev/null 2>&1
}
cleanup
: > "$LOG"

docker network create --subnet 172.30.0.0/24 $NET >/dev/null || fail "network create"

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
rpc_listen = "127.0.0.1:41100"
storage_dir = "/data/storage"
TOML

run_node() { # name ip
  # the entrypoint re-runs on every `docker start`: when /data/block-underlay
  # exists (written before the cold restart), the peer's TCP underlay is
  # REJECTed before the node comes up — a restart into a world with no
  # usable TCP ingress, which only the persisted mesh can escape.
  docker run -d --name "dtwg-$1" --network $NET --ip "$2" \
    --cap-add NET_ADMIN --device /dev/net/tun \
    -v dt-target:/target -v "$SCRATCH/$1":/data \
    rust:1.95 bash -c '
      apt-get update -qq >/dev/null 2>&1 &&
      apt-get install -y -qq iproute2 wireguard-tools openresolv iptables procps >/dev/null 2>&1 &&
      if [ -f /data/block-underlay ]; then
        PEER=$(cat /data/block-underlay) &&
        iptables -A OUTPUT -d "$PEER" -p tcp -j REJECT &&
        iptables -A INPUT -s "$PEER" -p tcp -j REJECT;
      fi &&
      exec /target/debug/ducktape-node --config /data/node.toml' >/dev/null || fail "start $1"
}

run_node node0 "$IP0"
run_node node1 "$IP1"

wait_marker() { # container marker timeout
  local deadline=$(( $(date +%s) + $3 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if docker logs "$1" 2>&1 | grep -q "$2"; then return 0; fi
    sleep 2
  done
  echo "== logs $1 ==" >> "$LOG"; docker logs "$1" >> "$LOG" 2>&1
  fail "$1 never printed: $2"
}

note "waiting for real tunnels on both nodes (1 peer each — a 0-peer apply is a FAILED epoch)"
wait_marker dtwg-node0 "tunnels applied on dt-.*(1 peer" 480
wait_marker dtwg-node1 "tunnels applied on dt-.*(1 peer" 480
note "tunnels applied with peers"

height() { # container
  docker exec "$1" bash -c \
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

note "baseline liveness (heights advance pre-cut)"
H0=$(wait_height_past dtwg-node0 2 120) || fail "node0 height stuck pre-cut"
H1=$(wait_height_past dtwg-node1 2 120) || fail "node1 height stuck pre-cut"
note "pre-cut heights: node0=$H0 node1=$H1"

note "cutting underlay TCP both directions (WG UDP stays open)"
docker exec dtwg-node0 bash -c \
  "iptables -A OUTPUT -d $IP1 -p tcp -j REJECT && iptables -A INPUT -s $IP1 -p tcp -j REJECT" \
  || fail "iptables node0"
docker exec dtwg-node1 bash -c \
  "iptables -A OUTPUT -d $IP0 -p tcp -j REJECT && iptables -A INPUT -s $IP0 -p tcp -j REJECT" \
  || fail "iptables node1"

note "waiting for mesh to re-dial over the tunnel and consensus to resume"
HA=$(wait_height_past dtwg-node0 $(( H0 + 3 )) 180) || fail "node0 did not advance after the underlay cut"
HB=$(wait_height_past dtwg-node1 $(( H1 + 3 )) 180) || fail "node1 did not advance after the underlay cut"
note "post-cut heights: node0=$HA node1=$HB — consensus rides the tunnel"

note "evidence: established mesh connections on the overlay"
docker exec dtwg-node0 ss -6 -t state established | tee -a "$LOG"
docker exec dtwg-node0 ip -6 addr show | grep -A1 "dt-" | tee -a "$LOG"
echo "PHASE 1-3 PASS: mesh traffic flows over the WireGuard tunnel after the underlay cut"

# ---- phase 4: whole-network cold restart with the underlay blocked from boot ----

note "cold restart: stopping BOTH nodes (tunnels die with the processes)"
docker stop dtwg-node0 dtwg-node1 >/dev/null || fail "stop"
echo "$IP1" > "$SCRATCH/node0/block-underlay"
echo "$IP0" > "$SCRATCH/node1/block-underlay"

[ -f "$SCRATCH/node0/storage/mesh-state.json" ] || fail "node0 never persisted its mesh"
[ -f "$SCRATCH/node1/storage/mesh-state.json" ] || fail "node1 never persisted its mesh"
note "persisted mesh state present on both nodes"

docker start dtwg-node0 dtwg-node1 >/dev/null || fail "restart"

wait_marker dtwg-node0 "persisted mesh (epoch .*) restored on dt-" 240
wait_marker dtwg-node1 "persisted mesh (epoch .*) restored on dt-" 240
note "both nodes restored tunnels from disk with zero TCP paths"

# node1 has a configured hint for node0 (config wins — no seed); node0 has
# no hint for node1, so its dial path to node1 IS the persisted ULA seed.
wait_marker dtwg-node0 "1 mesh dial seed(s) from the persisted mesh" 60
note "node0 seeded its dialer from the persisted mesh"

# docker logs span both lives: the live re-assembly after restart is the
# SECOND "tunnels applied" on each node.
wait_marker_count() { # container marker count timeout
  local deadline=$(( $(date +%s) + $4 ))
  while [ "$(date +%s)" -lt "$deadline" ]; do
    if [ "$(docker logs "$1" 2>&1 | grep -cF "$2")" -ge "$3" ]; then return 0; fi
    sleep 3
  done
  echo "== logs $1 ==" >> "$LOG"; docker logs "$1" >> "$LOG" 2>&1
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
docker exec dtwg-node0 iptables -L OUTPUT -n | tee -a "$LOG"
docker exec dtwg-node0 ss -6 -t state established | tee -a "$LOG"

cleanup
echo "SMOKE PASS: cold restart healed from the persisted mesh with no TCP ingress"
