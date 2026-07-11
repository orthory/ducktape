#!/usr/bin/env bash
# make demo-seed — a self-contained "demo" network preloaded with sample data.
#
# Inits a solo (1-validator) workspace named "demo" in ~/.ducktape — the SAME
# registry the desktop app reads — starts its node briefly, POSTs a batch of
# seed ops over the node's /v1/submit lane (each finalized into DURABLE qmdb
# state), then stops the node. Open the app and switch to the "demo" workspace:
# the app respawns the node from the same durable dir, fully populated.
#
# Re-runnable: wipes and recreates the "demo" workspace each time (other
# workspaces in the registry are untouched). Ports are freshly allocated.
#
# It also publishes two gateway web-app routes (see ops/demo-gateway.mjs): a
# NETWORK-hosted static site served from DuckFS, and a USER-hosted route that
# proxies to a node-local server. The frameless /v1/submit lane stamps the
# node's own validator key as the op origin, so the local daemon binds an
# Identity account and publishes routes as itself — the same path the app takes.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ID="${DEMO_WORKSPACE_ID:-demo}"
DUCK="$HOME/.ducktape"
WSDIR="$DUCK/workspaces/$ID"
REG="$DUCK/registry.json"
ORIGIN="demo"   # external author stamped on seeded ops (chat rejects an empty author)

log(){ printf '\033[36m[demo-seed]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-seed] %s\033[0m\n' "$*" >&2; exit 1; }

command -v bun     >/dev/null || die "bun is required"
command -v curl    >/dev/null || die "curl is required"

# ── 1. node binary ─────────────────────────────────────────────
NODE_BIN="${DUCKTAPE_NODE_BIN:-}"
if [ -z "$NODE_BIN" ]; then
  log "building ducktape-node (cargo build -p node-bin)…"
  blog="$(mktemp)"
  cargo build -p node-bin >"$blog" 2>&1 || die "node-bin build failed — see $blog"
  NODE_BIN="$(cargo metadata --no-deps --format-version 1 \
    | bun -e 'console.log((await Bun.stdin.json()).target_directory)')/debug/ducktape-node"
fi
[ -x "$NODE_BIN" ] || die "node binary not executable: $NODE_BIN"

# ── 2. fresh demo workspace (idempotent) ───────────────────────
log "creating a fresh '$ID' workspace at $WSDIR"
rm -rf "$WSDIR"; mkdir -p "$WSDIR"
read -r P1 P2 P3 < <(bun "$SCRIPT_DIR/fleet.mjs" ports 3)
# A free UDP port for the overlay's WireGuard socket. This MUST be concrete: the
# reachability plane refuses to start on port 0 ("wireguard_listen needs a
# concrete UDP port — plane not started"), which leaves the overlay down.
WGP="$(bun -e 'const s=require("node:dgram").createSocket("udp4");s.bind(0,"127.0.0.1",()=>{console.log(s.address().port);s.close()})')"
# Gateway serving needs the app's workspace_create posture:
#   --gateway            binds the isolated browser plane that serves the routes
#   --wireguard-effect   the userspace (TUN-less) overlay — no /dev/net/tun, no
#     socket             privilege; the default "tun" effect can't start without them
#   --wireguard-listen   a CONCRETE UDP port (0.0.0.0 = endpoint-less/roaming,
#     0.0.0.0:$WGP       like the app), so the overlay comes up instead of being
#                        skipped on port 0
#   --primary-coordinator a self-contained local demo does NOT phone home to the
#     none               public rendezvous coordinator; keeps network.toml (which
#                        the app reboots from) fully local.
CHAIN="$("$NODE_BIN" init --name "$ID" --dir "$WSDIR" \
  --listen 127.0.0.1:$P1 --advertised 127.0.0.1:$P1 \
  --http 127.0.0.1:$P2 --rpc 127.0.0.1:$P3 --gateway 127.0.0.1:0 \
  --primary-coordinator none \
  --wireguard-effect socket --wireguard-listen 0.0.0.0:$WGP 2>/dev/null | tail -1)"
[ -n "$CHAIN" ] || die "init produced no chain-id"
PUB="$("$NODE_BIN" keygen --out "$WSDIR/identity.key" 2>/dev/null | tail -1)"

# ── 3. register in ~/.ducktape/registry.json (merge; make it active) ──
bun - "$REG" "$ID" "$CHAIN" "$PUB" "$P1" "$P2" "$P3" <<'JS'
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
const [path, id, chain, pubkey, listen, http, rpc] = process.argv.slice(2);
let registry = { version: 1, active: null, workspaces: [] };
if (existsSync(path)) {
  try { registry = JSON.parse(readFileSync(path, "utf8")); } catch {}
}
const workspace = {
  id, name: id, chainId: chain, pubkey, founder: true, member: true,
  ports: { listen: Number(listen), http: Number(http), rpc: Number(rpc) },
};
registry.workspaces = [...(registry.workspaces ?? []).filter((item) => item.id !== id), workspace];
registry.active = id;
mkdirSync(dirname(path), { recursive: true });
writeFileSync(path, JSON.stringify(registry, null, 2));
JS
log "registered '$ID' (chain $CHAIN) — set as active workspace"

# ── 4. start the node, wait for its http surface ───────────────
log "starting node (http 127.0.0.1:$P2)…"
"$NODE_BIN" --config "$WSDIR/node.toml" >"$WSDIR/seed.log" 2>&1 &
NODE_PID=$!
trap 'kill "$NODE_PID" 2>/dev/null; wait "$NODE_PID" 2>/dev/null' EXIT
URL="http://127.0.0.1:$P2"
for _ in $(seq 1 80); do
  curl -sf "$URL/v1/status" >/dev/null 2>&1 && break
  kill -0 "$NODE_PID" 2>/dev/null || die "node exited on start — see $WSDIR/seed.log"
  sleep 0.5
done
curl -sf "$URL/v1/status" >/dev/null 2>&1 || die "node http never came up — see $WSDIR/seed.log"

# ── 5. seed ops ────────────────────────────────────────────────
N=0
submit(){ # submit <module> <payload-json>
  N=$((N+1))
  local body resp code
  body=$(bun -e 'const [target,payload,origin]=process.argv.slice(1);console.log(JSON.stringify({target,payload:JSON.parse(payload),origin}))' "$1" "$2" "$ORIGIN") \
    || die "op #$N ($1): payload is not valid json"
  resp=$(curl -s -w $'\n%{http_code}' "$URL/v1/submit" -H 'content-type: application/json' -d "$body")
  code=${resp##*$'\n'}
  [ "$code" = "200" ] || die "op #$N ($1) rejected [$code]: ${resp%$'\n'*}"
}

log "seeding modules…"

# pages — the Pages surface: a welcome page with a few blocks
submit pages '{"create_page":{"page_id":"welcome","title":"Welcome to Ducktape","parent":null}}'
submit pages '{"insert_block":{"parent":"welcome","after":null,"block":{"id":"w-h","kind":"heading2","text":"This is a demo network"}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-h","block":{"id":"w-p","kind":"paragraph","text":"Everything here was preloaded by make demo-seed. Poke around — chat, tasks, pages, agents."}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-p","block":{"id":"w-t1","kind":"todo","text":"Open the general channel"}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-t1","block":{"id":"w-t2","kind":"todo","text":"Check the tasks board"}}}'
submit pages '{"create_page":{"page_id":"runbook","title":"Team Runbook","parent":null}}'
submit pages '{"insert_block":{"parent":"runbook","after":null,"block":{"id":"rb-p","kind":"paragraph","text":"How we ship: branch off dev, PR, review, merge."}}}'

# chat — channels + messages + a reaction + an agent mention
submit chat '{"create_channel":{"channel_id":"general","name":"General","post_policy":"open"}}'
submit chat '{"create_channel":{"channel_id":"engineering","name":"Engineering","post_policy":"open"}}'
submit chat '{"create_channel":{"channel_id":"product","name":"Product","post_policy":"open"}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g1","blocks":[{"paragraph":[{"text":"Welcome to the demo network 👋","marks":[]}]}],"thread":null,"as_agent":null}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g2","blocks":[{"paragraph":[{"text":"This whole workspace is seeded — messages, tasks, pages and an agent.","marks":[]}]}],"thread":null,"as_agent":null}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g3","blocks":[{"paragraph":[{"text":"Nice, it even threads.","marks":[]}]}],"thread":1,"as_agent":null}}'
submit chat '{"add_reaction":{"channel_id":"general","seq":1,"emoji":"🦆"}}'
submit chat '{"post_message":{"channel_id":"engineering","message_id":"e1","blocks":[{"paragraph":[{"text":"CI is green on dev.","marks":[]}]}],"thread":null,"as_agent":null}}'
submit chat '{"post_message":{"channel_id":"product","message_id":"p1","blocks":[{"paragraph":[{"text":"Demo script for the deck is ready.","marks":[]}]}],"thread":null,"as_agent":null}}'

# tasks — a small board with mixed statuses
submit tasks '{"create_task":{"task_id":"t1","title":"Draft the launch announcement"}}'
submit tasks '{"create_task":{"task_id":"t2","title":"Review the onboarding flow"}}'
submit tasks '{"create_task":{"task_id":"t3","title":"Fix flaky identity test"}}'
submit tasks '{"update_status":{"task_id":"t2","status":"in_progress"}}'
submit tasks '{"update_status":{"task_id":"t3","status":"done"}}'

# agent — register a demo agent, watch general for @mentions, then mention it
submit agent '{"register_agent":{"agent_id":"quackbot","display_name":"Quackbot","capability":"mock-llm-1","prompt_hash":[7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7],"allowed_actions":["chat.post","tasks.create"]}}'
submit runs '{"watch_channel":{"channel_id":"general","policy":"mention"}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g4","blocks":[{"paragraph":[{"text":"hey ","marks":[]},{"text":"@quackbot","marks":[{"mention":{"agent":{"module":"runs","agent_id":"quackbot"}}}]},{"text":" can you follow up?","marks":[]}]}],"thread":null,"as_agent":null}}'

# jobs — a job on the board
submit jobs '{"submit":{"job_id":"j1","kind":"demo","spec":"render the welcome deck"}}'

# inbox — a starter notification for the demo author
submit inbox '{"deliver":{"member":"demo","kind":"welcome","body":"Your demo network is ready."}}'

# automations — a rule that files a task whenever someone says "deploy"
submit automations '{"create_rule":{"rule_id":"deploy-watch","trigger":{"message_posted":{"channel_id":null,"mention":null,"text_contains":"deploy"}},"action":{"create_task":{"task_id_prefix":"deploy","title_template":"Follow up on a deploy mention"}}}}'

# gateway — publish two web-app routes. The helper binds an Identity account to
# this node, stages the static site into DuckFS, and signs + submits both routes:
#   • site — a NETWORK-hosted static app, served from DuckFS by consensus
#   • app  — a USER-hosted app the gateway proxies to a node-local server
bun "$SCRIPT_DIR/demo-gateway.mjs" "$URL" "$NODE_BIN" "$WSDIR" "$CHAIN" "$ID" \
  || die "gateway route publish failed"

log "seeded $N ops + 2 gateway web-app routes across pages, chat, tasks, agent, runs, jobs, inbox, automations, files, gateway"

# ── 6. stop the node (state is durable on disk) ────────────────
kill "$NODE_PID" 2>/dev/null; wait "$NODE_PID" 2>/dev/null; trap - EXIT

cat <<EOF

$(printf '\033[32m[demo-seed] done.\033[0m')
Open the Ducktape app and it boots into the "$ID" workspace, preloaded.

Gateway web apps published on this node (open in the app's browser):
  • site.$ID.duck — a bouncing-DVD web app, served static from DuckFS by
                    consensus. Works now — nothing else to run.
  • app.$ID.duck  — user-hosted web app. The route is published, but its upstream
                    is a plain process YOU host. It shows "Unavailable" in the
                    Gateway view until you serve it, and goes Unavailable again if
                    that process stops:

                        make demo-app        # keep this running (foreground)

                    Re-run it after every \`make demo-seed\` (a re-seed wipes the
                    workspace and re-mints the loopback binding).
EOF
