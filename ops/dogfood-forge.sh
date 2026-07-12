#!/usr/bin/env bash
# make dogfood-forge — host ducktape's OWN source in ducktape's forge module.
#
# Registers a static git remote `ducktape-dev` pointing at the local dev node's
# forge git smart-HTTP endpoint, fetches the canonical development branch, then
# pushes that exact commit into Forge `main`. From then on, re-running this
# command refreshes Forge from the canonical source before any agent dispatch.
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
#   SOURCE_REMOTE           canonical source remote      (default: origin)
#   SOURCE_BRANCH           canonical source branch      (default: dev)
#   SRC_REF                 explicit local ref override  (default: fetched
#                                                        SOURCE_REMOTE/BRANCH)
#
# The default deliberately does NOT use HEAD. A clean-but-stale primary checkout
# can trail origin/dev while still looking healthy, which silently pins every
# later agent run to an obsolete source tree. An explicit SRC_REF remains useful
# for intentional branch dogfood, but callers then own that override.
#
# NOTE: `ducktape-dev` is a normal git remote, and git stores remotes in the
# SHARED .git/config (git-common-dir) — visible to every `git worktree` of this
# repo. If you run several worktrees each with their own node, they share this
# one remote; the script re-resolves and re-points it every run (and warns when
# the URL changes) so a run always targets the resolved node, but a stale
# `git push ducktape-dev` from another worktree could hit the wrong node. Set
# FORGE_REMOTE to a per-worktree name if you run many nodes at once.
set -euo pipefail
cd "$(dirname "$0")/.."

FORGE_REPO="${FORGE_REPO:-ducktape}"
FORGE_REMOTE="${FORGE_REMOTE:-ducktape-dev}"
SOURCE_REMOTE="${SOURCE_REMOTE:-origin}"
SOURCE_BRANCH="${SOURCE_BRANCH:-dev}"
SRC_REF="${SRC_REF:-}"

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

if [ -z "$SRC_REF" ]; then
  log "fetching canonical source: $SOURCE_REMOTE $SOURCE_BRANCH"
  git fetch "$SOURCE_REMOTE" "$SOURCE_BRANCH"
  # Resolve the result of THIS fetch, not a synthesized remote-tracking ref.
  # A remote with a missing/nonstandard fetch refspec may update FETCH_HEAD
  # while leaving refs/remotes/<remote>/<branch> stale.
  SRC_REF="FETCH_HEAD"
fi

SOURCE_OID="$(git rev-parse --verify "$SRC_REF^{commit}")" ||
  die "source ref '$SRC_REF' does not resolve to a commit"
log "source commit: $SOURCE_OID ($SRC_REF)"

# a healthy node is required (git-receive-pack is served off the node's http
# surface). fail fast with an actionable message rather than a git transport error.
if ! curl -fsS -m 5 "$BASE_URL/v1/status" >/dev/null 2>&1; then
  die "no node responding at $BASE_URL — start the dev app/node first \
(\`make dev\`), or set DUCKTAPE_DEV_FORGE_URL to a running node."
fi

# idempotent remote wiring: add, or re-point if it already exists.
if existing="$(git remote get-url "$FORGE_REMOTE" 2>/dev/null)"; then
  if [ "$existing" != "$REMOTE_URL" ]; then
    log "WARNING: '$FORGE_REMOTE' currently points at $existing"
    log "         re-pointing to $REMOTE_URL — this remote is SHARED across all git"
    log "         worktrees of this repo, so this also moves it for other worktrees."
  fi
  git remote set-url "$FORGE_REMOTE" "$REMOTE_URL"
  log "remote '$FORGE_REMOTE' -> $REMOTE_URL"
else
  git remote add "$FORGE_REMOTE" "$REMOTE_URL"
  log "added remote '$FORGE_REMOTE' -> $REMOTE_URL"
fi

# Forge only accepts refs/heads/main. Push the immutable OID verified above;
# do not resolve a mutable ref again after network and health checks have run.
log "pushing '$SOURCE_OID' -> $FORGE_REMOTE main (whole-repo pack over git-receive-pack)"
git push "$FORGE_REMOTE" "$SOURCE_OID:refs/heads/main"

# A successful git process is not enough evidence for the next dispatch. Read
# the committed Forge ref back through the same smart-HTTP boundary and require
# exact equality with the source commit we just selected.
FORGE_OID="$(git ls-remote "$REMOTE_URL" refs/heads/main | awk 'NR == 1 { print $1 }')"
if [ -z "$FORGE_OID" ]; then
  die "Forge main is missing after push"
fi
if [ "$FORGE_OID" != "$SOURCE_OID" ]; then
  die "Forge main verification failed: expected $SOURCE_OID, got $FORGE_OID"
fi

log "verified Forge main at $FORGE_OID"
log "done. ducktape now hosts the canonical dev source in Forge."
log "re-run \`make dogfood-forge\` before creating agent work."
