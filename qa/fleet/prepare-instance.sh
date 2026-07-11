#!/usr/bin/env bash
set -euo pipefail
umask 077

: "${FLEET_ARTIFACT_DIR:?Fleet must provide FLEET_ARTIFACT_DIR}"
: "${FLEET_HOME:?Fleet must provide FLEET_HOME}"
: "${FLEET_INSTANCE_ID:?Fleet must provide FLEET_INSTANCE_ID}"

node="$FLEET_ARTIFACT_DIR/bin/ducktape-node"
[ -x "$node" ] || { echo "Fleet artifact is missing ducktape-node" >&2; exit 1; }

workspace="$FLEET_HOME/.ducktape/workspaces/$FLEET_INSTANCE_ID"
registry="$FLEET_HOME/.ducktape/registry.json"
mkdir -p "$workspace"

read -r listen http rpc < <(bun -e '
  const listeners = Array.from({ length: 3 }, () => Bun.listen({ hostname: "127.0.0.1", port: 0, socket: { data() {} } }));
  console.log(listeners.map((listener) => listener.port).join(" "));
  listeners.forEach((listener) => listener.stop());
')

chain="$($node init --name "$FLEET_INSTANCE_ID" --dir "$workspace" \
  --listen "127.0.0.1:$listen" --advertised "127.0.0.1:$listen" \
  --http "127.0.0.1:$http" --rpc "127.0.0.1:$rpc" | tail -1)"
pubkey="$($node keygen --out "$workspace/identity.key" | tail -1)"

CHAIN="$chain" PUBKEY="$pubkey" LISTEN="$listen" HTTP="$http" RPC="$rpc" \
  bun -e '
    const id = process.env.FLEET_INSTANCE_ID;
    await Bun.write(process.argv[1], JSON.stringify({
      version: 1,
      active: id,
      workspaces: [{
        id,
        name: id,
        chainId: process.env.CHAIN,
        pubkey: process.env.PUBKEY,
        founder: true,
        member: true,
        ports: {
          listen: Number(process.env.LISTEN),
          http: Number(process.env.HTTP),
          rpc: Number(process.env.RPC)
        }
      }]
    }) + "\n");
  ' "$registry"
