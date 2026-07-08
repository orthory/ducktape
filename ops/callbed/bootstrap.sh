#!/usr/bin/env bash
# Peer the call-bed validators — OFFLINE. Runs the network-shape ceremony
# (init -> invite -> join -> admit -> invite -> join, same as
# bin/node/examples/demo-invite.sh) ONCE, writing each node's config dir into
# the shared volume (/shared/node0, /shared/node1). Each node container then
# runs `ducktape-node --config /shared/nodeK/node.toml`; they peer at runtime.
#
# Nodes BIND 0.0.0.0 but ADVERTISE their compose service name (node0/node1) so
# peers dial each other by DNS on the compose network. The whole ceremony is
# offline — the invite blob carries the descriptor, so no node need be running.
# Idempotent: existing configs => no-op (a re-`up` keeps the same identities).
set -euo pipefail
BIN=/usr/local/bin/ducktape-node
SH=/shared
P2P=9000 HTTP=8080 RPC=7070
A="$SH/node0" B="$SH/node1"

if [ -f "$A/node.toml" ] && [ -f "$B/node.toml" ]; then
  echo "[bootstrap] node0/node1 configs already present — nothing to do"; exit 0
fi
mkdir -p "$SH"

echo "[bootstrap] node0 (founder): init"
"$BIN" init --name callbed --dir "$A" \
  --listen 0.0.0.0:$P2P --advertised node0:$P2P --http 0.0.0.0:$HTTP --rpc 0.0.0.0:$RPC >/dev/null
inv=$("$BIN" invite --config "$A/node.toml")

echo "[bootstrap] node1 (friend): join (identity pass)"
fk=$("$BIN" join "$inv" --dir "$B" \
  --listen 0.0.0.0:$P2P --advertised node1:$P2P --http 0.0.0.0:$HTTP --rpc 0.0.0.0:$RPC)
echo "[bootstrap]   node1 key: $fk"

echo "[bootstrap] node0 admits node1 + refreshes invite"
"$BIN" admit "$fk" --config "$A/node.toml" >/dev/null
inv2=$("$BIN" invite --config "$A/node.toml")

echo "[bootstrap] node1: join (member pass)"
"$BIN" join "$inv2" --dir "$B" \
  --listen 0.0.0.0:$P2P --advertised node1:$P2P --http 0.0.0.0:$HTTP --rpc 0.0.0.0:$RPC >/dev/null

echo "[bootstrap] done — configs at $A and $B"
