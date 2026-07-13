#!/usr/bin/env bash
# Mirror one merged, canonical Forge PR onto GitHub dev without rewriting its
# feature commit messages or identities. GitHub receives a draft PR; review it,
# wait for checks, then merge it with `gh pr merge --merge` (never squash/rebase).
set -euo pipefail

log() { printf '\033[36m[forge-mirror]\033[0m %s\n' "$*"; }
warn() { printf '\033[33m[forge-mirror]\033[0m %s\n' "$*" >&2; }
die() { printf '\033[31m[forge-mirror]\033[0m %s\n' "$*" >&2; exit 1; }

is_oid() { [[ "$1" =~ ^[0-9a-fA-F]{40}$ ]]; }

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

query_item() {
  local out=$1
  curl --fail --silent --show-error --max-time 10 \
    -X POST "$BASE_URL/v1/query" \
    -H 'content-type: application/json' \
    --data "{\"target\":\"forge\",\"query\":{\"get_item\":{\"repo\":\"$FORGE_REPO\",\"number\":$PR_NUMBER}}}" \
    >"$out"
}

parse_item() {
  local json=$1 fields=$2 body=$3
  node - "$json" "$PR_NUMBER" "$SOURCE_OID" "$fields" "$body" <<'NODE'
const fs = require("node:fs");
const [jsonPath, numberText, sourceOid, fieldsPath, bodyPath] = process.argv.slice(2);
const reply = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
const item = reply.item;
const fail = message => { throw new Error(message); };
if (!item) fail(`Forge PR #${numberText} does not exist`);
if (item.number !== Number(numberText)) fail("Forge returned a different item number");
if (item.kind !== "pr" || item.state !== "merged") fail("only merged Forge PRs can be mirrored");
if (item.target_branch !== "main") fail("Forge PR target must be main");
if (typeof item.source_branch !== "string" || !item.source_branch) fail("Forge PR has no source branch");
if (!/^[0-9a-f]{40}$/i.test(item.merge_oid || "")) fail("Forge PR has no valid merge oid");
if (!/^[0-9a-f]{40}$/i.test(sourceOid)) fail("source commit must be 40 hex characters");
if (typeof item.title !== "string" || !item.title.trim()) fail("Forge PR title is empty");
if (/[\u0000-\u001f\u007f]/.test(item.title)) fail("Forge PR title must be one printable line");
fs.writeFileSync(fieldsPath, [item.source_branch, item.target_branch, item.merge_oid, item.title].join("\n") + "\n");
fs.writeFileSync(bodyPath, item.body || "");
NODE
}

neutralize_github_closing_keywords() {
  local body=$1 repo=$2
  node - "$body" "$repo" <<'NODE'
const fs = require("node:fs");
const [bodyPath, repo] = process.argv.slice(2);
const lines = fs.readFileSync(bodyPath, "utf8").split("\n");
let fence = "";

for (let i = 0; i < lines.length; i += 1) {
  const marker = lines[i].match(/^ {0,3}(`{3,}|~{3,})/)?.[1];
  if (fence) {
    if (marker?.[0] === fence[0] && marker.length >= fence.length &&
        new RegExp(`^ {0,3}\\${fence[0]}{${fence.length},}[ \\t]*$`).test(lines[i])) {
      fence = "";
    }
    continue;
  }
  if (marker) {
    fence = marker;
    continue;
  }
  lines[i] = lines[i].replace(
    /\b(?:close[sd]?|fix(?:es|ed)?|resolve[sd]?)\s+#([1-9][0-9]*)\b/gi,
    (match, number, offset) =>
      `${lines[i].slice(0, offset).trim() ? "addresses" : "Addresses"} Forge ${repo} item ${number}`,
  );
}

fs.writeFileSync(bodyPath, lines.join("\n"));
NODE
}

assert_clean() {
  local dir=$1
  [ -z "$(git -C "$dir" status --porcelain --untracked-files=all)" ] ||
    die "generated mirror worktree is dirty: $dir"
}

assert_cutover_mapping() {
  local forge_target=$1 github_actual=$2 github_expected=$3
  is_oid "$forge_target" && is_oid "$github_expected" || die "cutover mapping contains an invalid oid"
  [ "$github_actual" = "$github_expected" ] ||
    die "origin/dev is $github_actual, not the explicit cutover base $github_expected"
}

validate_merged_selection() {
  local merge=$1 source=$2
  is_oid "$merge" && is_oid "$source" || die "merge/source oid is invalid"
  local line
  line=$(git show -s --format=%P "$merge") || die "Forge merge commit $merge is unavailable"
  local -a parents
  read -r -a parents <<<"$line"
  [ "${#parents[@]}" -eq 2 ] || die "Forge merge $merge is not a two-parent PR merge"
  [ "${parents[1]}" = "$source" ] ||
    die "explicit source $source is not Forge merge $merge second parent (${parents[1]})"
  printf '%s\n' "${parents[0]}"
}

validate_commit_range() {
  local target=$1 source=$2
  mapfile -t SOURCE_COMMITS < <(git rev-list --reverse --topo-order "$target..$source")
  [ "${#SOURCE_COMMITS[@]}" -gt 0 ] || die "Forge PR contains no commits to mirror"
  local previous="" commit line
  local -a parents
  for commit in "${SOURCE_COMMITS[@]}"; do
    line=$(git show -s --format=%P "$commit")
    read -r -a parents <<<"$line"
    [ "${#parents[@]}" -eq 1 ] || die "Forge feature history contains merge commit $commit"
    if [ -z "$previous" ]; then
      git merge-base --is-ancestor "${parents[0]}" "$target" ||
        die "Forge feature history does not fork from the merged target"
    else
      [ "${parents[0]}" = "$previous" ] || die "Forge feature history is not linear at $commit"
    fi
    previous=$commit
  done
}

has_only_replayable_headers() {
  git cat-file commit "$1" | awk '
    /^$/ { exit }
    $1 == "tree" || $1 == "parent" || $1 == "author" || $1 == "committer" { next }
    { unsupported = 1 }
    END { exit unsupported }
  '
}

commit_fingerprint() {
  git show -s --format=format:'%an%x00%ae%x00%aI%x00%cn%x00%ce%x00%cI%x00' "$1"
}

commit_identity_headers() {
  git cat-file commit "$1" | sed -n '/^author /p; /^committer /p'
}

write_raw_message() {
  local oid=$1 raw=$2 message=$3
  git cat-file commit "$oid" >"$raw"
  node - "$raw" "$message" <<'NODE'
const fs = require("node:fs");
const [rawPath, messagePath] = process.argv.slice(2);
const commit = fs.readFileSync(rawPath);
const split = commit.indexOf("\n\n");
if (split < 0) throw new Error("commit has no header/message separator");
fs.writeFileSync(messagePath, commit.subarray(split + 2));
NODE
}

replay_commit() {
  local dir=$1 source=$2 state_dir=$3
  assert_clean "$dir"
  has_only_replayable_headers "$source" ||
    die "source commit $source has an encoding, signature, or unsupported header that cannot be preserved"

  git -C "$dir" cherry-pick --no-commit "$source" || die "source commit $source does not apply cleanly"
  git -C "$dir" diff --cached --quiet && die "source commit $source is empty on GitHub dev"

  local -a meta
  mapfile -d '' -t meta < <(commit_fingerprint "$source")
  [ "${#meta[@]}" -eq 6 ] || die "could not read source commit identity"

  local source_raw="$state_dir/source-commit" new_raw="$state_dir/new-commit"
  local source_message="$state_dir/source-message" new_message="$state_dir/new-message"
  local source_meta="$state_dir/source-meta" new_meta="$state_dir/new-meta"
  local source_patch="$state_dir/source-patch" new_patch="$state_dir/new-patch"
  write_raw_message "$source" "$source_raw" "$source_message"
  commit_identity_headers "$source" >"$source_meta"
  git show --pretty=format: --binary "$source" | git patch-id --stable >"$source_patch"
  [ -s "$source_patch" ] || die "source commit $source has no patch"

  local tree parent mirrored
  tree=$(git -C "$dir" write-tree)
  parent=$(git -C "$dir" rev-parse HEAD)
  mirrored=$(
    GIT_AUTHOR_NAME="${meta[0]}" \
    GIT_AUTHOR_EMAIL="${meta[1]}" \
    GIT_AUTHOR_DATE="${meta[2]}" \
    GIT_COMMITTER_NAME="${meta[3]}" \
    GIT_COMMITTER_EMAIL="${meta[4]}" \
    GIT_COMMITTER_DATE="${meta[5]}" \
      git -C "$dir" -c commit.gpgSign=false commit-tree "$tree" -p "$parent" <"$source_message"
  ) || die "git commit-tree failed for $source"
  is_oid "$mirrored" || die "git commit-tree returned an invalid oid"
  git -C "$dir" reset --hard "$mirrored" >/dev/null

  write_raw_message "$mirrored" "$new_raw" "$new_message"
  commit_identity_headers "$mirrored" >"$new_meta"
  git show --pretty=format: --binary "$mirrored" | git patch-id --stable >"$new_patch"
  cmp -s "$source_message" "$new_message" || die "raw commit message changed while replaying $source"
  cmp -s "$source_meta" "$new_meta" || die "raw author/committer identity changed while replaying $source"
  cmp -s "$source_patch" "$new_patch" || die "commit patch changed while replaying $source"
  assert_clean "$dir"
  MIRRORED_OID=$mirrored
}

remote_ref_oid() {
  local remote=$1 ref=$2 out rc
  set +e
  out=$(git ls-remote --exit-code "$remote" "$ref" 2>/dev/null)
  rc=$?
  set -e
  case "$rc" in
    0) awk 'NR == 1 { print $1 }' <<<"$out" ;;
    2) return 1 ;;
    *) die "could not read $remote $ref" ;;
  esac
}

cleanup() {
  local rc=$?
  trap - EXIT
  if [ "$rc" -ne 0 ] && [ "${PUSHED:-0}" -eq 1 ] && [ "${PR_CREATED:-0}" -eq 0 ]; then
    warn "preserving pushed branch refs/heads/$MIRROR_BRANCH for inspection or PR recovery"
  fi
  if [ "${WORKTREE_ADDED:-0}" -eq 1 ]; then
    git -C "$MIRROR_WORKTREE" cherry-pick --abort >/dev/null 2>&1 || true
    git worktree remove --force "$MIRROR_WORKTREE" >/dev/null 2>&1 ||
      warn "could not remove disposable worktree $MIRROR_WORKTREE"
  fi
  [ -z "${FORGE_TMP_REF:-}" ] || git update-ref -d "$FORGE_TMP_REF" >/dev/null 2>&1 || true
  [ -z "${ORIGIN_TMP_REF:-}" ] || git update-ref -d "$ORIGIN_TMP_REF" >/dev/null 2>&1 || true
  if [ -n "${RUN_DIR:-}" ] && [ -d "$RUN_DIR" ]; then
    rm -f "$RUN_DIR/item.json" "$RUN_DIR/item-recheck.json" "$RUN_DIR/item-fields" \
      "$RUN_DIR/pr-body" "$RUN_DIR/source-message" "$RUN_DIR/new-message" \
      "$RUN_DIR/source-commit" "$RUN_DIR/new-commit" \
      "$RUN_DIR/source-meta" "$RUN_DIR/new-meta" "$RUN_DIR/source-patch" \
      "$RUN_DIR/new-patch" "$RUN_DIR/gh.out" "$RUN_DIR/gh.err"
    rmdir "$RUN_DIR" >/dev/null 2>&1 || warn "temporary state remains at $RUN_DIR"
  fi
  exit "$rc"
}

main() {
  [ "$#" -eq 3 ] ||
    die "usage: ops/mirror-forge-pr.sh <forge-pr-number> <source-head-oid> <github-dev-oid-for-forge-target>"
  PR_NUMBER=$1
  SOURCE_OID=${2,,}
  EXPECTED_GITHUB_OID=${3,,}
  [[ "$PR_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "Forge PR number must be a positive integer"
  is_oid "$SOURCE_OID" || die "source commit must be 40 hex characters"
  is_oid "$EXPECTED_GITHUB_OID" || die "expected GitHub dev commit must be 40 hex characters"
  command -v curl >/dev/null || die "curl is required"
  command -v git >/dev/null || die "git is required"
  command -v node >/dev/null || die "node is required to parse Forge metadata"
  command -v gh >/dev/null || die "gh is required to open the GitHub draft PR"

  FORGE_REPO=${FORGE_REPO:-ducktape}
  [[ "$FORGE_REPO" =~ ^[a-z0-9._-]{1,64}$ ]] || die "FORGE_REPO is not a safe Forge repo slug"
  GH_REPO=orthory/ducktape
  local origin_url
  origin_url=$(git remote get-url --push origin) || die "origin has no push URL"
  case "$origin_url" in
    https://github.com/orthory/ducktape.git | git@github.com:orthory/ducktape.git) ;;
    *) die "origin push URL is not the orthory/ducktape GitHub mirror: $origin_url" ;;
  esac
  BASE_URL=$(resolve_base_url)
  FORGE_URL="$BASE_URL/forge/$FORGE_REPO"
  local common primary token short
  common=$(git rev-parse --path-format=absolute --git-common-dir)
  primary=$(dirname "$common")
  short=${SOURCE_OID:0:12}
  token="$PR_NUMBER-$short-$$"
  RUN_DIR="$primary/.worktree/.forge-mirror-$token"
  MIRROR_WORKTREE="$RUN_DIR/worktree"
  FORGE_TMP_REF="refs/mirror-tmp/$token/forge-main"
  ORIGIN_TMP_REF="refs/mirror-tmp/$token/origin-dev"
  MIRROR_BRANCH="mirror/forge-pr-$PR_NUMBER-$short"
  mkdir -p "$primary/.worktree"
  mkdir "$RUN_DIR"
  trap cleanup EXIT

  log "reading canonical Forge PR #$PR_NUMBER from $BASE_URL"
  query_item "$RUN_DIR/item.json"
  parse_item "$RUN_DIR/item.json" "$RUN_DIR/item-fields" "$RUN_DIR/pr-body"
  neutralize_github_closing_keywords "$RUN_DIR/pr-body" "$FORGE_REPO"
  local -a item
  mapfile -t item <"$RUN_DIR/item-fields"
  [ "${#item[@]}" -eq 4 ] || die "Forge PR metadata was incomplete"
  SOURCE_BRANCH=${item[0]}
  TARGET_BRANCH=${item[1]}
  MERGE_OID=${item[2],,}
  PR_TITLE=${item[3]}
  git check-ref-format --branch "$SOURCE_BRANCH" >/dev/null || die "Forge source branch is invalid"

  # The live Forge upload-pack reliably serves one wanted branch. Do not turn
  # this into a wildcard/all-refs fetch: its multi-want path is not supported.
  log "fetching Forge main at one unique temporary ref"
  git fetch --no-tags "$FORGE_URL" "refs/heads/main:$FORGE_TMP_REF"
  local forge_main
  forge_main=$(git rev-parse "$FORGE_TMP_REF^{commit}")
  git merge-base --is-ancestor "$MERGE_OID" "$forge_main" ||
    die "Forge merge $MERGE_OID is not in canonical Forge main"
  local forge_target
  forge_target=$(validate_merged_selection "$MERGE_OID" "$SOURCE_OID")
  validate_commit_range "$forge_target" "$SOURCE_OID"

  log "fetching exact GitHub dev base"
  git fetch --no-tags origin "refs/heads/dev:$ORIGIN_TMP_REF"
  local origin_oid
  origin_oid=$(git rev-parse "$ORIGIN_TMP_REF^{commit}")
  # Reparenting destroys ancestry, so Git cannot infer this relationship. The
  # third required argument is the operator's explicit cutover assertion that
  # this Forge target's prerequisites are represented by this exact GitHub dev.
  assert_cutover_mapping "$forge_target" "$origin_oid" "$EXPECTED_GITHUB_OID"
  log "explicit cutover mapping: Forge target $forge_target -> GitHub dev $origin_oid"
  git worktree add --detach "$MIRROR_WORKTREE" "$origin_oid" >/dev/null
  WORKTREE_ADDED=1
  assert_clean "$MIRROR_WORKTREE"

  local commit
  for commit in "${SOURCE_COMMITS[@]}"; do
    log "replaying $commit with its original message and identities"
    replay_commit "$MIRROR_WORKTREE" "$commit" "$RUN_DIR"
  done
  local mirror_tip=$MIRRORED_OID
  git -C "$MIRROR_WORKTREE" diff --quiet "$origin_oid..$mirror_tip" &&
    die "Forge PR has no net changes on GitHub dev"
  git -C "$MIRROR_WORKTREE" diff --check "$origin_oid..$mirror_tip"

  gh auth status -h github.com >/dev/null 2>&1 ||
    die "gh is not authenticated for github.com (authenticate before mirroring)"
  local remote_dev
  remote_dev=$(remote_ref_oid origin refs/heads/dev) || die "origin/dev is missing"
  [ "$remote_dev" = "$origin_oid" ] || die "origin/dev moved during replay; retry from the new base"
  if remote_ref_oid origin "refs/heads/$MIRROR_BRANCH" >/dev/null; then
    die "GitHub mirror branch already exists: $MIRROR_BRANCH"
  fi

  cat >>"$RUN_DIR/pr-body" <<EOF


---
Canonical Forge PR: $FORGE_REPO#$PR_NUMBER
Forge merge: $MERGE_OID
Forge source: $SOURCE_OID
Forge target: $forge_target
GitHub base: $origin_oid

This GitHub PR is a mirror. Merge it with a merge commit; do not squash or rebase.
EOF

  log "pushing $mirror_tip to $MIRROR_BRANCH"
  # An absent-ref lease makes branch ownership atomic even when two runs race
  # with the same deterministic branch and commit (a plain push may say that
  # the second identical push is already up to date).
  git -C "$MIRROR_WORKTREE" push \
    --force-with-lease="refs/heads/$MIRROR_BRANCH:" \
    origin "HEAD:refs/heads/$MIRROR_BRANCH"
  PUSHED=1
  local pushed_oid
  pushed_oid=$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH") || die "pushed branch is missing"
  [ "$pushed_oid" = "$mirror_tip" ] || die "pushed branch does not match the verified mirror commit"
  remote_dev=$(remote_ref_oid origin refs/heads/dev) || die "origin/dev disappeared after push"
  [ "$remote_dev" = "$origin_oid" ] || die "origin/dev moved while pushing; retry from the new base"

  query_item "$RUN_DIR/item-recheck.json"
  cmp -s "$RUN_DIR/item.json" "$RUN_DIR/item-recheck.json" ||
    die "Forge PR metadata changed during mirroring; retry"

  set +e
  gh pr create --draft --repo "$GH_REPO" --base dev --head "$MIRROR_BRANCH" \
    --title "$PR_TITLE" --body-file "$RUN_DIR/pr-body" \
    >"$RUN_DIR/gh.out" 2>"$RUN_DIR/gh.err"
  local gh_rc=$?
  set -e
  if [ "$gh_rc" -ne 0 ]; then
    local existing="" list_rc
    set +e
    existing=$(gh pr list --repo "$GH_REPO" --state all --head "$MIRROR_BRANCH" \
      --json url --jq '.[0].url // ""' 2>/dev/null)
    list_rc=$?
    set -e
    if [ "$list_rc" -eq 0 ] && [ -n "$existing" ]; then
      PR_CREATED=1
      die "gh lost the create result, but the draft PR exists: $existing"
    fi
    die "gh pr create failed: $(tr '\n' ' ' <"$RUN_DIR/gh.err")"
  fi

  local pr_url
  pr_url=$(tail -n 1 "$RUN_DIR/gh.out")
  [ -n "$pr_url" ] || die "gh created a PR but returned no URL"
  PR_CREATED=1
  log "draft PR: $pr_url"
  log "after review and green checks: gh pr merge --repo $GH_REPO --merge --delete-branch $pr_url"
}

if [ "${BASH_SOURCE[0]}" = "$0" ]; then
  main "$@"
fi
