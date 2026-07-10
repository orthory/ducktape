#!/usr/bin/env bash
# Node container entrypoint: wait for bootstrap to have written this node's
# config into the shared volume, then run the validator. `depends_on:
# service_completed_successfully` already gates on bootstrap exiting, so the
# wait is just belt-and-braces against volume-mount races.
set -euo pipefail
CFG="${1:?usage: node-entry.sh <config-dir>}"
for _ in $(seq 1 120); do [ -f "$CFG/node.toml" ] && break; sleep 0.5; done
[ -f "$CFG/node.toml" ] || { echo "[node] $CFG/node.toml never appeared"; exit 1; }
# Rewrite the WireGuard bind to THIS container's concrete address, every boot
# (container IPs are not stable across runs). An unspecified wireguard_listen
# (0.0.0.0) deliberately means "advertise NO endpoint" — the NAT'd-roaming
# default — so with it, two pre-admitted members deadlock waiting for each
# other to initiate (#331). A concrete IP makes the node advertise
# `ip:51820` in its signed EndpointRecord; the peer learns it over the TCP
# mesh gossip and initiates, and one dialable side is enough (WireGuard
# roaming pins the reverse path).
IP="$(hostname -i | awk '{print $1}')"
sed -i "s|^wireguard_listen = \"[^\"]*:|wireguard_listen = \"$IP:|" "$CFG/node.toml"
echo "[node] wireguard_listen bound to concrete $IP (endpoint advertised in mesh records)"
echo "[node] launching ducktape-node --config $CFG/node.toml"
exec /usr/local/bin/ducktape-node --config "$CFG/node.toml"
