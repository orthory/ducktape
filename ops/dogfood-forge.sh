#!/usr/bin/env bash
# make dogfood-forge — host ducktape's OWN source in ducktape's forge module.
# This flows GitHub origin/dev -> Forge dev without moving release-only main.
#
# Registers a static git remote `ducktape-dev` pointing at the local dev node's
# forge git smart-HTTP endpoint, fetches the canonical development branch, then
# synchronizes that exact history into Forge `dev`. Re-running this command
# refreshes Forge before agent work. A fast-forward is direct; equal-tree
# mirror divergence is joined with a two-parent bridge; differing trees fail
# closed for a reviewed reconciliation.
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
  die "no Forge node selected; set DUCKTAPE_DEV_FORGE_URL or start an active workspace"
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
  # NOTE: no backticks in this string — it is double-quoted, so they would be
  # command substitution, and the die message would RUN whatever it names.
  die "no node responding at $BASE_URL — start a node first \
(make install-node, once, to fill ~/.ducktape/modules with the components its \
genesis composes from; then cargo run -p noded-bin), or set \
DUCKTAPE_DEV_FORGE_URL to a running node."
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

FORGE_REF=refs/heads/dev
FORGE_OID="$(git ls-remote "$REMOTE_URL" "$FORGE_REF" | awk 'NR == 1 { print $1 }')"
EXPECTED_OID=$SOURCE_OID

if [ -z "$FORGE_OID" ]; then
  log "creating Forge dev at $SOURCE_OID"
  git push "$FORGE_REMOTE" "$SOURCE_OID:$FORGE_REF"
else
  TMP_REF="refs/dogfood-sync/$$/forge-dev"
  trap 'git update-ref -d "$TMP_REF" >/dev/null 2>&1 || true' EXIT
  git fetch --no-tags "$FORGE_REMOTE" "$FORGE_REF:$TMP_REF"
  if [ "$FORGE_OID" = "$SOURCE_OID" ]; then
    log "Forge dev already matches GitHub dev"
  elif git merge-base --is-ancestor "$FORGE_OID" "$SOURCE_OID"; then
    log "fast-forwarding Forge dev to GitHub dev"
    git push "$FORGE_REMOTE" "$SOURCE_OID:$FORGE_REF"
  elif git merge-base --is-ancestor "$SOURCE_OID" "$FORGE_OID"; then
    log "Forge dev already contains GitHub dev"
    EXPECTED_OID=$FORGE_OID
  elif git diff --quiet "$FORGE_OID" "$SOURCE_OID"; then
    command -v node >/dev/null || die "node is required to read the node identity"
    NODE_ID=$(
      curl -fsS -m 5 "$BASE_URL/v1/status" |
        node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{const k=JSON.parse(s).public_key||"";if(!/^[0-9a-f]{64}$/i.test(k))process.exit(1);process.stdout.write(k.toLowerCase())})'
    ) || die "the node status has no valid public_key"
    TREE_OID=$(git rev-parse "$SOURCE_OID^{tree}")
    EXPECTED_OID=$(
      GIT_AUTHOR_NAME="$NODE_ID" \
      GIT_AUTHOR_EMAIL="$NODE_ID@nodes.duck" \
      GIT_COMMITTER_NAME="$NODE_ID" \
      GIT_COMMITTER_EMAIL="$NODE_ID@nodes.duck" \
        git commit-tree "$TREE_OID" -p "$FORGE_OID" -p "$SOURCE_OID" <<EOF
Synchronize GitHub dev into Forge dev

Join provenance-equivalent development histories without rewriting either side.
EOF
    )
    log "joining provenance-equivalent dev histories at $EXPECTED_OID"
    git push "$FORGE_REMOTE" "$EXPECTED_OID:$FORGE_REF"
  else
    die "Forge dev $FORGE_OID and GitHub dev $SOURCE_OID diverged with different trees; reconcile them in a reviewed PR"
  fi
fi

# A successful git process is not enough evidence for the next dispatch.
VERIFIED_OID="$(git ls-remote "$REMOTE_URL" "$FORGE_REF" | awk 'NR == 1 { print $1 }')"
if [ "$VERIFIED_OID" != "$EXPECTED_OID" ]; then
  die "Forge dev verification failed: expected $EXPECTED_OID, got ${VERIFIED_OID:-missing}"
fi

log "verified Forge dev at $VERIFIED_OID"
log "release-only Forge main was not changed."
log "re-run \`make dogfood-forge\` before creating agent work."
