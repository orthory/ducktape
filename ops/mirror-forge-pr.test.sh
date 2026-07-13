#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
REPO_ROOT=$PWD
# shellcheck source=ops/mirror-forge-pr.sh
source ops/mirror-forge-pr.sh

common=$(git rev-parse --path-format=absolute --git-common-dir)
TEST_ROOT="$(dirname "$common")/.worktree/.forge-mirror-test-$$"
SOURCE_REPO="$TEST_ROOT/source"
DEST_REPO="$TEST_ROOT/destination"
CONFLICT_REPO="$TEST_ROOT/conflict"
mkdir -p "$TEST_ROOT"
trap 'rm -rf "$TEST_ROOT"' EXIT

quiet_git() { git -C "$1" "${@:2}" >/dev/null; }
configure() {
  quiet_git "$1" config user.name Test
  quiet_git "$1" config user.email test@example.com
}

git init -b dev "$SOURCE_REPO" >/dev/null
configure "$SOURCE_REPO"
printf 'base\n' >"$SOURCE_REPO/shared.txt"
quiet_git "$SOURCE_REPO" add shared.txt
quiet_git "$SOURCE_REPO" commit -m 'base'
BASE=$(git -C "$SOURCE_REPO" rev-parse HEAD)

quiet_git "$SOURCE_REPO" switch -c agent/item-26
printf 'forge\n' >"$SOURCE_REPO/shared.txt"
printf 'mirrored feature\n' >"$SOURCE_REPO/feature.txt"
quiet_git "$SOURCE_REPO" add shared.txt feature.txt
printf 'fix(forge): preserve agent provenance\n\nKeep this body verbatim.\n\nNo Apply agent changes fallback.\n' \
  >"$TEST_ROOT/message"
GIT_AUTHOR_NAME='Agent Alice' \
GIT_AUTHOR_EMAIL='alice@agents.duck' \
GIT_AUTHOR_DATE='2026-07-01T01:02:03+09:00' \
GIT_COMMITTER_NAME='node-alpha' \
GIT_COMMITTER_EMAIL='node-alpha@nodes.duck' \
GIT_COMMITTER_DATE='2026-07-01T04:05:06+09:00' \
  git -C "$SOURCE_REPO" commit -F "$TEST_ROOT/message" >/dev/null
SOURCE_OID=$(git -C "$SOURCE_REPO" rev-parse HEAD)

quiet_git "$SOURCE_REPO" switch dev
printf 'internal dev advanced\n' >"$SOURCE_REPO/internal.txt"
quiet_git "$SOURCE_REPO" add internal.txt
quiet_git "$SOURCE_REPO" commit -m 'internal-only advance'
quiet_git "$SOURCE_REPO" merge --no-ff agent/item-26 -m 'Merge Forge PR #26'
MERGE_OID=$(git -C "$SOURCE_REPO" rev-parse HEAD)
TARGET_OID=$(git -C "$SOURCE_REPO" rev-parse HEAD^1)

git init -b dev "$DEST_REPO" >/dev/null
configure "$DEST_REPO"
quiet_git "$DEST_REPO" fetch "$SOURCE_REPO" "$BASE"
quiet_git "$DEST_REPO" reset --hard FETCH_HEAD
# One dev fetch supplies the merged PR and its second-parent feature closure.
quiet_git "$DEST_REPO" fetch "$SOURCE_REPO" refs/heads/dev:refs/mirror-test/forge-dev
quiet_git "$DEST_REPO" reset --hard "$TARGET_OID"
printf 'GitHub dev advanced independently\n' >"$DEST_REPO/github.txt"
quiet_git "$DEST_REPO" add github.txt
quiet_git "$DEST_REPO" commit -m 'github-only advance'
GITHUB_BASE=$(git -C "$DEST_REPO" rev-parse HEAD)

cd "$DEST_REPO"
selected_target=$(validate_merged_selection "$MERGE_OID" "$SOURCE_OID")
[ "$selected_target" = "$TARGET_OID" ]
validate_commit_range "$selected_target" "$SOURCE_OID"
[ "${#SOURCE_COMMITS[@]}" -eq 1 ]
[ "${SOURCE_COMMITS[0]}" = "$SOURCE_OID" ]

mkdir "$TEST_ROOT/state"
replay_commit "$DEST_REPO" "$SOURCE_OID" "$TEST_ROOT/state"
MIRROR_OID=$MIRRORED_OID
[ "$(git rev-parse "$MIRROR_OID^1")" = "$GITHUB_BASE" ]
[ "$(git show "$MIRROR_OID:feature.txt")" = 'mirrored feature' ]
cmp <(git show -s --format=%B "$SOURCE_OID") <(git show -s --format=%B "$MIRROR_OID")
cmp <(commit_fingerprint "$SOURCE_OID") <(commit_fingerprint "$MIRROR_OID")
cmp \
  <(git show --pretty=format: --binary "$SOURCE_OID" | git patch-id --stable) \
  <(git show --pretty=format: --binary "$MIRROR_OID" | git patch-id --stable)
[ "$(git show -s --format=%ce "$MIRROR_OID")" = 'node-alpha@nodes.duck' ]
assert_shared_dev_base "$TARGET_OID" "$GITHUB_BASE"
bridge_tree=$(git rev-parse "$GITHUB_BASE^{tree}")
BRIDGE=$(printf 'history-only bridge\n' | git commit-tree "$bridge_tree" -p "$GITHUB_BASE")
assert_shared_dev_base "$BRIDGE" "$GITHUB_BASE"
ancestor_tree=$(git rev-parse "$BASE^{tree}")
ANCESTOR_TREE_BRIDGE=$(
  printf 'Forge history bridge with a GitHub ancestor tree\n' |
    git commit-tree "$ancestor_tree" -p "$MERGE_OID"
)
assert_shared_dev_base "$ANCESTOR_TREE_BRIDGE" "$GITHUB_BASE"
if (assert_shared_dev_base "$TARGET_OID" "$BASE") >/dev/null 2>&1; then
  echo 'Forge-only content outside GitHub dev was accepted' >&2
  exit 1
fi
if (assert_shared_dev_base "$TARGET_OID" "$SOURCE_OID") >/dev/null 2>&1; then
  echo 'sibling Forge and GitHub histories were accepted' >&2
  exit 1
fi

printf '%s\n' \
  'Closes #49. This fixes #12 and RESOLVED #13.' \
  'A plain #77 stays unchanged.' \
  '````markdown' \
  'Closes #98 inside a fence.' \
  '```' \
  'Fixes #99 after a short fence marker.' \
  '````' \
  'This fixed #14 outside the fence.' \
  >"$TEST_ROOT/body"
neutralize_github_closing_keywords "$TEST_ROOT/body" ducktape
printf '%s\n' \
  'Addresses Forge ducktape item 49. This addresses Forge ducktape item 12 and addresses Forge ducktape item 13.' \
  'A plain #77 stays unchanged.' \
  '````markdown' \
  'Closes #98 inside a fence.' \
  '```' \
  'Fixes #99 after a short fence marker.' \
  '````' \
  'This addresses Forge ducktape item 14 outside the fence.' \
  >"$TEST_ROOT/expected-body"
cmp "$TEST_ROOT/expected-body" "$TEST_ROOT/body"

if (validate_merged_selection "$MERGE_OID" "$BASE") >/dev/null 2>&1; then
  echo 'explicit source mismatch was accepted' >&2
  exit 1
fi
if (validate_commit_range "$BASE" "$MERGE_OID") >/dev/null 2>&1; then
  echo 'merge commit in feature history was accepted' >&2
  exit 1
fi
tree=$(git rev-parse "$BASE^{tree}")
for header in 'encoding ISO-8859-1' 'gpgsig fake-signature' 'x-extra unsupported'; do
  unsupported=$(
    printf 'tree %s\nparent %s\nauthor Agent <agent@example.com> 1 +0000\ncommitter node <node@nodes.duck> 1 +0000\n%s\n\nmessage\n' \
      "$tree" "$BASE" "$header" | git hash-object -t commit -w --stdin
  )
  if has_only_replayable_headers "$unsupported"; then
    echo "commit with non-replayable header was accepted: $header" >&2
    exit 1
  fi
done
printf 'dirty\n' >"$DEST_REPO/untracked"
if (assert_clean "$DEST_REPO") >/dev/null 2>&1; then
  echo 'dirty generated worktree was accepted' >&2
  exit 1
fi
rm "$DEST_REPO/untracked"

git init -b dev "$CONFLICT_REPO" >/dev/null
configure "$CONFLICT_REPO"
quiet_git "$CONFLICT_REPO" fetch "$SOURCE_REPO" "$BASE"
quiet_git "$CONFLICT_REPO" reset --hard FETCH_HEAD
printf 'github conflict\n' >"$CONFLICT_REPO/shared.txt"
quiet_git "$CONFLICT_REPO" add shared.txt
quiet_git "$CONFLICT_REPO" commit -m 'conflicting GitHub change'
quiet_git "$CONFLICT_REPO" fetch "$SOURCE_REPO" refs/heads/dev:refs/mirror-test/forge-dev
mkdir "$TEST_ROOT/conflict-state"
if (cd "$CONFLICT_REPO" && replay_commit "$CONFLICT_REPO" "$SOURCE_OID" "$TEST_ROOT/conflict-state") \
  >/dev/null 2>&1; then
  echo 'conflicting source commit was accepted' >&2
  exit 1
fi

grep -F 'refs/heads/dev:$FORGE_TMP_REF' "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null
if grep -F 'refs/heads/*' "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null; then
  echo 'Forge wildcard fetch must not be used' >&2
  exit 1
fi
grep -F -- '--force-with-lease="refs/heads/$MIRROR_BRANCH:"' \
  "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null
if grep -F 'origin ":refs/heads/$MIRROR_BRANCH"' "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null; then
  echo 'pushed mirror branches must never be deleted automatically' >&2
  exit 1
fi

printf 'mirror-forge-pr: all checks passed\n'
