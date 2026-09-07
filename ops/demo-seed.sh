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
# node's own validator key as the op origin. Its account controls the model
# user; the separate demo wallet signs and owns the gateway routes.
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
# no wasm is embedded in the binary: founding composes the workspace genesis
# out of the founding set the build staged beside the binary (`<target>/
# <profile>/modules`), which `node init` finds by itself — nothing to point at.
ID="${DEMO_WORKSPACE_ID:-demo}"
# The SAME root the CLI resolves (`wallet::duck_root`) and the app resolves
# (`duck_home`). Hardcoding `$HOME` split the two under DUCKTAPE_HOME: the
# guard below and the gateway's signing key looked at `$HOME/.ducktape/keys`
# while `wallet new` minted into `$DUCKTAPE_HOME/keys`, so a second run died
# on "already exists" and the gateway signed with the wrong key.
DUCK="${DUCKTAPE_HOME:-$HOME/.ducktape}"
WSDIR="$DUCK/workspaces/$ID"
REG="$DUCK/registry.json"
USERKEY="$DUCK/keys/demo.key"     # the app signs writes with THIS local key
DEMO_PASSWORD="${DEMO_KEY_PASSWORD:-ducktape}"  # unlock password for the demo identity

# DEV_LISTEN widens the p2p mesh + HTTP API binds so a second machine can
# reach this node: default stays 127.0.0.1 (a localhost-only dev loop);
# DEV_LISTEN=0.0.0.0 (or [::]) binds wide. The WireGuard plane is bound wide
# regardless (see --wireguard-listen below).
DEV_LISTEN="${DEV_LISTEN:-127.0.0.1}"
# advertised is the dial-hint peers actually use, so widening `listen` alone
# leaves a second machine learning a loopback address it cannot dial (#1240
# half of #1241) — reaching this node from box B needs
#   DEV_LISTEN=0.0.0.0 DEV_ADVERTISED=<this box's LAN ip>
DEV_ADVERTISED="${DEV_ADVERTISED:-127.0.0.1}"

log(){ printf '\033[36m[demo-seed]\033[0m %s\n' "$*"; }
die(){ printf '\033[31m[demo-seed] %s\033[0m\n' "$*" >&2; exit 1; }

command -v bun     >/dev/null || die "bun is required"
command -v curl    >/dev/null || die "curl is required"

# ── 1. node binary ─────────────────────────────────────────────
NODE_BIN="${DUCKTAPE_NODE_BIN:-}"
if [ -z "$NODE_BIN" ]; then
  log "building ducktape (cargo build -p node-bin)…"
  blog="$(mktemp)"
  cargo build -p node-bin >"$blog" 2>&1 || die "node-bin build failed — see $blog"
  NODE_BIN="$(cargo metadata --no-deps --format-version 1 \
    | bun -e 'console.log((await Bun.stdin.json()).target_directory)')/debug/ducktape"
fi
[ -x "$NODE_BIN" ] || die "node binary not executable: $NODE_BIN"

# ── 2. fresh demo workspace (idempotent) ───────────────────────
log "creating a fresh '$ID' workspace at $WSDIR"
rm -rf "$WSDIR"; mkdir -p "$WSDIR"
# Free-port probe only — always loopback regardless of DEV_LISTEN, since it
# never binds anything the node itself serves from.
read -r P1 P2 P3 < <(bun -e 'const l=Array.from({length:3},()=>Bun.listen({hostname:"127.0.0.1",port:0,socket:{data(){}}}));process.stdout.write(l.map(x=>x.port).join(" ")+"\n");l.forEach(x=>x.stop())')
# A free UDP port for the overlay's WireGuard socket. This MUST be concrete: the
# reachability plane refuses to start on port 0 ("wireguard_listen needs a
# concrete UDP port — plane not started"), which leaves the overlay down.
WGP="$(bun -e 'const s=await Bun.udpSocket({port:0});process.stdout.write(String(s.port));s.close()')"
[[ "$WGP" =~ ^[0-9]+$ ]] || die "UDP port allocation produced non-numeric output"
# Gateway serving needs the app's workspace_create posture:
#   --listen             p2p mesh bind: DEV_LISTEN (#1241) — a second machine
#                        needs this wide to dial in for a peer join or huddle
#   --advertised         the dial-hint address WE announce to peers, not a
#     $DEV_ADVERTISED    bind — DEV_ADVERTISED (default 127.0.0.1), a
#                        SEPARATE knob from DEV_LISTEN: a wide `listen` with
#                        a loopback `advertised` still hands box B a dial
#                        hint it cannot use, so reaching this node from a
#                        second machine needs BOTH DEV_LISTEN=0.0.0.0 and
#                        DEV_ADVERTISED=<this box's LAN ip>
#   --http               the HTTP app API, widened with the mesh: reads are
#     $DEV_LISTEN        open to any peer and writes are signed, so a second
#                        machine's app can point straight at this node
#   --rpc                127.0.0.1     local operator rpc — the CLI on THIS
#                        box only, no peer or huddle plane reads it, stays
#                        loopback always
#   --gateway             binds the isolated browser plane that serves the routes;
#     127.0.0.1:0         the node refuses any bind but 127.0.0.1 for it
#   --wireguard-listen   a CONCRETE UDP port, already bound wide (0.0.0.0 =
#     0.0.0.0:$WGP       endpoint-less/roaming, like the app) regardless of
#                        DEV_LISTEN, so the overlay a peer join/huddle needs
#                        already reaches a second machine
#   --primary-coordinator a self-contained local demo does NOT phone home to the
#     none               public rendezvous coordinator; keeps network.toml (which
#                        the app reboots from) fully local.
INIT_ERR="$(mktemp)"
if ! CHAIN="$("$NODE_BIN" node init --name "$ID" --dir "$WSDIR" \
  --listen "$DEV_LISTEN:$P1" --advertised "$DEV_ADVERTISED:$P1" \
  --http "$DEV_LISTEN:$P2" --rpc "127.0.0.1:$P3" --gateway 127.0.0.1:0 \
  --primary-coordinator none \
  --wireguard-listen "0.0.0.0:$WGP" 2>"$INIT_ERR" | tail -1)"; then
  sed -n '1,120p' "$INIT_ERR" >&2
  rm -f "$INIT_ERR"
  die "node init failed"
fi
rm -f "$INIT_ERR"
[ -n "$CHAIN" ] || die "init produced no chain-id"
PUB="$("$NODE_BIN" node key --out "$WSDIR/identity.key" 2>/dev/null | tail -1)"

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

# ── 3b. user identity ──────────────────────────────────────────
# The app signs writes with a wallet from the keystore. The demo gets its
# OWN named wallet ("demo", password $DEMO_PASSWORD) so the seed always
# holds the signing password: the old "existing key, unknown password,
# routes skipped" branch cannot happen. The user's other wallets are
# untouched; the seed never flips the active pointer — the app's wallet
# list is where the demo identity gets picked.
if [ -e "$USERKEY" ]; then
  log "demo wallet already present at $USERKEY"
else
  printf '%s\n' "$DEMO_PASSWORD" | "$NODE_BIN" wallet new demo >/dev/null \
    || die "could not mint the demo wallet"
  log "minted the demo wallet (password: $DEMO_PASSWORD)"
fi

# ── 4. start the node, wait for its http surface ───────────────
log "starting node (http $DEV_LISTEN:$P2)…"
"$NODE_BIN" node run --config "$WSDIR/node.toml" >"$WSDIR/seed.log" 2>&1 &
NODE_PID=$!
trap 'kill "$NODE_PID" 2>/dev/null; wait "$NODE_PID" 2>/dev/null' EXIT
# This script always talks to the node it just started, from THIS box — a
# wildcard bind still accepts loopback, so the seeder's own curl calls stay
# on 127.0.0.1 regardless of DEV_LISTEN.
URL="http://127.0.0.1:$P2"
for _ in $(seq 1 80); do
  curl -sf "$URL/v1/status" >/dev/null 2>&1 && break
  kill -0 "$NODE_PID" 2>/dev/null || die "node exited on start — see $WSDIR/seed.log"
  sleep 0.5
done
curl -sf "$URL/v1/status" >/dev/null 2>&1 || die "node http never came up — see $WSDIR/seed.log"

# ── 4b. grant the compute service ──────────────────────────────
# The compute plane is consent-gated: a [sandbox] table says HOW a run would
# be isolated, and the user's grant says WHETHER this node runs any. There is
# no init flag for it any more, so the demo mints the grant the same way an
# operator does — `service run` discovers this host's providers, signals them,
# and --enable grants from that live hello. It only needs to run long enough
# for the grant to land, so it is stopped once services.toml appears.
"$NODE_BIN" service run compute --enable --workspace "$WSDIR" >"$WSDIR/service.log" 2>&1 &
SVC_PID=$!
for _ in $(seq 1 50); do [ -f "$WSDIR/services.toml" ] && break; sleep 0.1; done
kill "$SVC_PID" 2>/dev/null; wait "$SVC_PID" 2>/dev/null
if [ -f "$WSDIR/services.toml" ]; then
  log "compute granted — agent runs available"
else
  # The reason is in service.log and nowhere else; guessing at it ("no usable
  # container runtime?") sends the reader hunting for the wrong thing — the
  # usual cause is a node.toml with no live [sandbox] table, not a missing VMM.
  log "compute NOT granted:"
  tail -n 3 "$WSDIR/service.log" 2>/dev/null | sed 's/^/    /'
  log "  full log: $WSDIR/service.log"
  log "  the demo still runs, just without agent runs. Grant it later with:"
  log "  ducktape service run compute --workspace $WSDIR"
fi

# ── 5. seed ops ────────────────────────────────────────────────
# Every mutating /v1 route wants a credential: either a per-request user
# signature or this node's own operator credential, minted 0600 into the
# workspace at boot. The seeder acts as the node's operator, so it presents
# that one — a bare curl is no longer a way in.
OPERATOR="$(cat "$WSDIR/admin.token" 2>/dev/null)" \
  || die "the node minted no operator credential at $WSDIR/admin.token"
[ -n "$OPERATOR" ] || die "the node minted no operator credential at $WSDIR/admin.token"
N=0
submit(){ # submit <module> <payload-json>
  N=$((N+1))
  local body resp code
  body=$(bun -e 'const [target,payload]=process.argv.slice(1);console.log(JSON.stringify({target,payload:JSON.parse(payload)}))' "$1" "$2") \
    || die "op #$N ($1): payload is not valid json"
  resp=$(curl -s -w $'\n%{http_code}' "$URL/v1/submit" -H 'content-type: application/json' \
    -H "x-ducktape-admin-token: $OPERATOR" -d "$body")
  code=${resp##*$'\n'}
  [ "$code" = "200" ] || die "op #$N ($1) rejected [$code]: ${resp%$'\n'*}"
}

log "seeding modules…"

# pages — the Pages surface: a welcome page with a few blocks
submit pages '{"create_page":{"page_id":"welcome","title":"Welcome to Ducktape"}}'
submit pages '{"insert_block":{"parent":"welcome","after":null,"block":{"id":"w-h","kind":"heading2","text":"This is a demo network"}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-h","block":{"id":"w-p","kind":"paragraph","text":"Everything here was preloaded by make demo-seed. Poke around — chat, tasks, pages, agents."}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-p","block":{"id":"w-t1","kind":"todo","text":"Open the general channel"}}}'
submit pages '{"insert_block":{"parent":"welcome","after":"w-t1","block":{"id":"w-t2","kind":"todo","text":"Check the tasks board"}}}'
submit pages '{"create_page":{"page_id":"runbook","title":"Team Runbook"}}'
submit pages '{"insert_block":{"parent":"runbook","after":null,"block":{"id":"rb-p","kind":"paragraph","text":"How we ship: branch off dev, PR, review, merge."}}}'

# chat — channels + messages + a reaction + an agent mention
submit chat '{"create_channel":{"channel_id":"general","name":"General","post_policy":"open"}}'
submit chat '{"create_channel":{"channel_id":"engineering","name":"Engineering","post_policy":"open"}}'
submit chat '{"create_channel":{"channel_id":"product","name":"Product","post_policy":"open"}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g1","blocks":[{"paragraph":[{"text":"Welcome to the demo network 👋","marks":[]}]}],"thread":null}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g2","blocks":[{"paragraph":[{"text":"This whole workspace is seeded — messages, tasks, pages and an agent.","marks":[]}]}],"thread":null}}'
submit chat '{"post_message":{"channel_id":"general","message_id":"g3","blocks":[{"paragraph":[{"text":"Nice, it even threads.","marks":[]}]}],"thread":1}}'
submit chat '{"add_reaction":{"channel_id":"general","seq":1,"emoji":"🦆"}}'
submit chat '{"post_message":{"channel_id":"engineering","message_id":"e1","blocks":[{"paragraph":[{"text":"CI is green on dev.","marks":[]}]}],"thread":null}}'
submit chat '{"post_message":{"channel_id":"product","message_id":"p1","blocks":[{"paragraph":[{"text":"Demo script for the deck is ready.","marks":[]}]}],"thread":null}}'

# tasks — a small board with mixed statuses. both boards ride ONE wire envelope
# (WorkMsg): task-board ops are wrapped `{"task":{…}}`, job-board ops `{"job":{…}}`.
submit tasks '{"task":{"create_task":{"task_id":"t1","title":"Draft the launch announcement"}}}'
submit tasks '{"task":{"create_task":{"task_id":"t2","title":"Review the onboarding flow"}}}'
submit tasks '{"task":{"create_task":{"task_id":"t3","title":"Fix flaky identity test"}}}'
submit tasks '{"task":{"update_status":{"task_id":"t2","status":"in_progress"}}}'
submit tasks '{"task":{"update_status":{"task_id":"t3","status":"done"}}}'

# model user — the operator account controls a keyless programmable account.
# The recipe is emitted by the current binary, never copied into this script.
query(){ # query <module> <query-json>
  local body
  body=$(bun -e 'const [target,query]=process.argv.slice(1);process.stdout.write(JSON.stringify({target,query:JSON.parse(query)}))' "$1" "$2") || die "invalid query"
  curl -fsS "$URL/v1/query" -H 'content-type: application/json' -d "$body" || die "query failed"
}
NODE_BYTES=$(bun -e 'process.stdout.write(JSON.stringify([...Buffer.from(process.argv[1],"hex")]))' "$PUB")
CONTROLLER=$(query identity "{\"of_key\":{\"key\":$NODE_BYTES}}" | bun -e 'process.stdout.write(String((await Bun.stdin.json()).account?.number ?? ""))')
if [ -z "$CONTROLLER" ]; then
  submit identity '{"create":{"name":"Demo operator","scheme":"ed25519"}}'
  CONTROLLER=$(query identity "{\"of_key\":{\"key\":$NODE_BYTES}}" | bun -e 'process.stdout.write(String((await Bun.stdin.json()).account?.number ?? ""))')
fi
[ -n "$CONTROLLER" ] || die "the operator has no controller account"
PROGRAM=$("$NODE_BIN" agent model-program quackbot) || die "cannot encode the default model program"
PROVISION=$(printf '%s' "$PROGRAM" | bun -e 'process.stdout.write(JSON.stringify({provision:{name:"Quackbot",program:await Bun.stdin.json()}}))') || die "invalid program"
submit agent "$PROVISION"
MODEL_ACCOUNT=$(query identity "{\"controlled\":{\"by\":$CONTROLLER,\"from\":0,\"limit\":256}}" | bun -e '
  const matches=(await Bun.stdin.json()).accounts.filter(account=>account.name==="Quackbot" && account.control.program?.executor==="agent");
  if(matches.length!==1) throw new Error("expected exactly one Quackbot program account");
  process.stdout.write(String(matches[0].number));
') || die "cannot resolve the model account"
submit runs "{\"configure_model\":{\"operation\":{\"register_model\":{\"account\":$MODEL_ACCOUNT,\"agent_id\":\"quackbot\",\"display_name\":\"Quackbot\",\"capability\":\"mock-llm-1\",\"allowed_actions\":[\"chat.post\",\"tasks.create\"]}}}}"
MENTION=$(bun -e 'process.stdout.write(JSON.stringify({post_message:{channel_id:"general",message_id:"g4",blocks:[{paragraph:[{text:"@quackbot can you follow up?",marks:[{mention:{account:Number(process.argv[1])}}]}]}],thread:null}}))' "$MODEL_ACCOUNT")
submit chat "$MENTION"

# jobs — a job on the board. the job board shares the "tasks" target under the
# WorkMsg `{"job":{…}}` arm (there is no separate "jobs" module).
submit tasks '{"job":{"submit":{"job_id":"j1","kind":"demo","spec":"render the welcome deck"}}}'

# inbox — a starter notification for the demo author
submit inbox '{"deliver":{"member":"demo","kind":"welcome","body":"Your demo network is ready."}}'

# automations — a rule that files a task whenever someone says "deploy"
submit automations '{"create_rule":{"rule_id":"deploy-watch","trigger":{"channel_id":null,"mention":null,"text_contains":"deploy"},"action":{"create_task":{"task_id_prefix":"deploy","title_template":"Follow up on a deploy mention"}}}}'

# gateway — publish three web-app routes. The helper binds an Identity account to
# this node, stages the static site into DuckFS, and signs + submits the routes:
#   • site — a NETWORK-hosted static app, served from DuckFS by consensus
#   • app  — a USER-hosted app the gateway proxies to a node-local server
#   • board — the network-visible kanban reference app
# Sign the routes with the demo wallet the seed itself minted — the seed
# always holds its password, so the sign step always runs (though failure is
# non-fatal; chat/tasks/pages are already durable regardless).
GATEWAY_ROUTES=3
GATEWAY_PW="$DEMO_PASSWORD"
bun "$SCRIPT_DIR/demo-gateway.mjs" "$URL" "$NODE_BIN" "$WSDIR" "$CHAIN" "$ID" "$USERKEY" "$GATEWAY_PW"
gateway_status=$?
# Route publishing is a demo garnish — its failure never kills the seed. The
# core workspace (chat, tasks, pages, identity) is committed before this runs.
case "$gateway_status" in
  0) ;;
  *)
    GATEWAY_ROUTES=0
    log "gateway routes skipped (exit $gateway_status) — see $WSDIR/seed.log"
    ;;
esac

log "seeded $N ops + $GATEWAY_ROUTES gateway web-app routes across pages, chat, tasks, agent, runs, jobs, inbox, automations, files, gateway"

# ── 6. stop the node (state is durable on disk) ────────────────
kill "$NODE_PID" 2>/dev/null; wait "$NODE_PID" 2>/dev/null; trap - EXIT

cat <<EOF

$(printf '\033[32m[demo-seed] done.\033[0m')
Open the Ducktape app and it boots into the "$ID" workspace, preloaded.

To WRITE (send a message, add a reaction, edit): the app signs with a wallet
from your keystore. The launch window opens on the wallet list — pick the
"demo" row, type its password into that row, and Unlock. That also makes it
the active wallet, so the next launch opens on it.

  wallet: demo   password: $DEMO_PASSWORD

EOF

if [ "$GATEWAY_ROUTES" -eq 0 ]; then
cat <<EOF
Gateway web apps were not published — see $WSDIR/seed.log for the exact
rejection. This is a demo garnish only: chat, tasks, pages and your identity are
all live.
EOF
else
cat <<EOF
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
fi

cat <<EOF
Done with the demo? \`make demo-clear\` removes the workspace entirely.
EOF
