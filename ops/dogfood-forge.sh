#!/usr/bin/env bash
# make dogfood-forge — host ducktape's OWN source in ducktape's forge module.
#
# Registers a static git remote `ducktape-dev` pointing at the local dev node's
# forge git smart-HTTP endpoint, then pushes `main` into it. From then on,
# dogfooding is just `git push ducktape-dev main` — real ducktape history flows
# into ducktape's own forge, browsable in the desktop Forge view.
#
# This is the INTENDED big-repo path: `git-receive-pack` lifts the body cap to
# 512 MB and stores the whole packfile node-locally, submitting only a tiny
# `forge Push` (32-byte digest + oids) through consensus — the pack NEVER crosses
# consensus. (Contrast POST /v1/files/blob, which is capped at the 4 MB chunk
# size and 413s a whole-repo pack.)
#
# Resolution of the node's forge base URL, in order:
#   1. $DUCKTAPE_DEV_FORGE_URL           — explicit base, e.g. http://127.0.0.1:8844
#   2. the ACTIVE workspace's http_listen — ~/.ducktape/registry.json (.active)
#                                            -> ~/.ducktape/workspaces/<active>/node.toml
#      (the workspace flow assigns a RANDOM http port, so this is not a fixed :8844)
#   3. http://127.0.0.1:8844             — the web/legacy default
#
# Env knobs:
#   DUCKTAPE_DEV_FORGE_URL  node base URL override (no trailing /forge/<repo>)
#   FORGE_REPO              forge repo name in the URL   (default: ducktape)
#   FORGE_REMOTE            local git remote name        (default: ducktape-dev)
#   SRC_REF                 local ref pushed to main     (default: main)
set -euo pipefail
cd "$(dirname "$0")/.."

FORGE_REPO="${FORGE_REPO:-ducktape}"
FORGE_REMOTE="${FORGE_REMOTE:-ducktape-dev}"
SRC_REF="${SRC_REF:-main}"

log() { printf '\033[36m[dogfood]\033[0m %s\n' "$*"; }
die() { printf '\033[31m[dogfood]\033[0m %s\n' "$*" >&2; exit 1; }

resolve_base_url() {
  if [ -n "${DUCKTAPE_DEV_FORGE_URL:-}" ]; then
    printf '%s' "${DUCKTAPE_DEV_FORGE_URL%/}"
    return
  fi
  local reg="$HOME/.ducktape/registry.json"
  if [ -f "$reg" ]; then
    local active
    active=$(sed -n 's/.*"active"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$reg" | head -1)
    if [ -n "$active" ]; then
      local toml="$HOME/.ducktape/workspaces/$active/node.toml"
      if [ -f "$toml" ]; then
        local listen
        listen=$(sed -n 's/^[[:space:]]*http_listen[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$toml" | head -1)
        if [ -n "$listen" ]; then
          printf 'http://%s' "$listen"
          return
        fi
      fi
    fi
  fi
  printf 'http://127.0.0.1:8844'
}

BASE_URL="$(resolve_base_url)"
REMOTE_URL="$BASE_URL/forge/$FORGE_REPO"

log "node forge endpoint: $REMOTE_URL"

# a healthy node is required (git-receive-pack is served off the node's http
# surface). fail fast with an actionable message rather than a git transport error.
if ! curl -fsS -m 5 "$BASE_URL/v1/status" >/dev/null 2>&1; then
  die "no node responding at $BASE_URL — start the dev app/node first \
(\`make dev\`), or set DUCKTAPE_DEV_FORGE_URL to a running node."
fi

# idempotent remote wiring: add, or re-point if it already exists.
if git remote get-url "$FORGE_REMOTE" >/dev/null 2>&1; then
  git remote set-url "$FORGE_REMOTE" "$REMOTE_URL"
  log "re-pointed remote '$FORGE_REMOTE' -> $REMOTE_URL"
else
  git remote add "$FORGE_REMOTE" "$REMOTE_URL"
  log "added remote '$FORGE_REMOTE' -> $REMOTE_URL"
fi

# forge only accepts refs/heads/main; push whatever SRC_REF names into it.
log "pushing '$SRC_REF' -> $FORGE_REMOTE main (whole-repo pack over git-receive-pack)"
git push "$FORGE_REMOTE" "$SRC_REF:refs/heads/main"

log "done. ducktape now hosts itself in forge — browse it in the desktop Forge view."
log "re-run \`make dogfood-forge\` (or \`git push $FORGE_REMOTE main\`) to update."
