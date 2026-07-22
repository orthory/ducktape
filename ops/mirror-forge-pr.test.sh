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
verify_replayed_commit "$SOURCE_OID" "$MIRROR_OID" "$TEST_ROOT" 'initial replay'
[ "$(git show -s --format=%ce "$MIRROR_OID")" = 'node-alpha@nodes.duck' ]

# Epic identifiers accept either a full safe branch or a generic slug. Unsafe
# refs are rejected before any remote read or write.
normalize_epic_branch 'improvement/platform-quality'
[ "$EPIC_BRANCH" = 'improvement/platform-quality' ]
normalize_epic_branch 'platform-quality'
[ "$EPIC_BRANCH" = 'improvement/platform-quality' ]
if (normalize_epic_branch '../unsafe') >/dev/null 2>&1; then
  echo 'unsafe epic branch was accepted' >&2
  exit 1
fi
if (normalize_epic_branch 'refs/heads/dev') >/dev/null 2>&1; then
  echo 'fully qualified epic ref was accepted' >&2
  exit 1
fi

# A long-lived epic keeps its original first-parent ledger even when origin/dev
# advances independently.
history_tree=$(git rev-parse "$GITHUB_BASE^{tree}")
EPIC_HISTORY_TIP=$(printf 'epic slice\n' | git commit-tree "$history_tree" -p "$GITHUB_BASE")
ADVANCED_DEV=$(printf 'independent dev advance\n' | git commit-tree "$history_tree" -p "$GITHUB_BASE")
[ "$(resolve_epic_history_base "$ADVANCED_DEV" "$EPIC_HISTORY_TIP")" = "$GITHUB_BASE" ]

# A leading Forge merge may checkpoint GitHub history already represented by
# current dev. Replay it as a two-parent epic commit while preserving its tree,
# message, identities, and original GitHub second parent.
checkpoint_message="$TEST_ROOT/checkpoint-message"
printf 'Synchronize represented GitHub dev\n\nKeep this merge body verbatim.\n' >"$checkpoint_message"
CHECKPOINT_SOURCE=$(
  GIT_AUTHOR_NAME='Sync Agent' GIT_AUTHOR_EMAIL='sync@agents.duck' \
  GIT_AUTHOR_DATE='2026-07-03T01:02:03+00:00' GIT_COMMITTER_NAME='Sync Node' \
  GIT_COMMITTER_EMAIL='sync@nodes.duck' GIT_COMMITTER_DATE='2026-07-03T04:05:06+00:00' \
    git -c commit.gpgSign=false commit-tree "$GITHUB_BASE^{tree}" \
      -p "$TARGET_OID" -p "$GITHUB_BASE" <"$checkpoint_message"
)
CHECKPOINT_CORRECTION=$(
  printf 'Correct the synchronized client\n' | \
    git commit-tree "$SOURCE_OID^{tree}" -p "$CHECKPOINT_SOURCE"
)
validate_commit_range "$TARGET_OID" "$CHECKPOINT_CORRECTION" "$GITHUB_BASE"
[ "${SOURCE_COMMITS[*]}" = "$CHECKPOINT_SOURCE $CHECKPOINT_CORRECTION" ]

EPIC_PRE_CHECKPOINT=$(
  printf 'verified epic before checkpoint\n' | \
    git commit-tree "$TARGET_OID^{tree}" -p "$BASE"
)
quiet_git "$DEST_REPO" reset --hard "$EPIC_PRE_CHECKPOINT"
mkdir "$TEST_ROOT/checkpoint-state"
replay_commit "$DEST_REPO" "$CHECKPOINT_SOURCE" "$TEST_ROOT/checkpoint-state" "$GITHUB_BASE"
CHECKPOINT_MIRROR=$MIRRORED_OID
[ "$(git show -s --format=%P "$CHECKPOINT_MIRROR")" = \
  "$EPIC_PRE_CHECKPOINT $GITHUB_BASE" ]
[ "$(git rev-parse "$CHECKPOINT_MIRROR^{tree}")" = \
  "$(git rev-parse "$CHECKPOINT_SOURCE^{tree}")" ]
verify_replayed_commit "$CHECKPOINT_SOURCE" "$CHECKPOINT_MIRROR" \
  "$TEST_ROOT/checkpoint-state" 'checkpoint ledger repeat' "$GITHUB_BASE"
replay_commit "$DEST_REPO" "$CHECKPOINT_CORRECTION" "$TEST_ROOT/checkpoint-state" \
  "$GITHUB_BASE"
CHECKPOINT_CORRECTION_MIRROR=$MIRRORED_OID
[ "$(resolve_epic_history_base "$GITHUB_BASE" "$CHECKPOINT_CORRECTION_MIRROR")" = "$BASE" ]
[ "$(git rev-list --first-parent --reverse "$BASE..$CHECKPOINT_CORRECTION_MIRROR" | \
  paste -sd ' ' -)" = \
  "$EPIC_PRE_CHECKPOINT $CHECKPOINT_MIRROR $CHECKPOINT_CORRECTION_MIRROR" ]

OUTSIDE_GITHUB=$(
  printf 'outside current GitHub dev\n' | git commit-tree "$BASE^{tree}" -p "$BASE"
)
BAD_CHECKPOINT=$(
  printf 'bad checkpoint\n' | git commit-tree "$GITHUB_BASE^{tree}" \
    -p "$TARGET_OID" -p "$OUTSIDE_GITHUB"
)
if (validate_commit_range "$TARGET_OID" "$BAD_CHECKPOINT" "$GITHUB_BASE") \
  >/dev/null 2>&1; then
  echo 'checkpoint with an unrepresented GitHub parent was accepted' >&2
  exit 1
fi
NONLEADING_BASE=$(
  printf 'ordinary first commit\n' | git commit-tree "$TARGET_OID^{tree}" -p "$TARGET_OID"
)
NONLEADING_MERGE=$(
  printf 'late merge\n' | git commit-tree "$GITHUB_BASE^{tree}" \
    -p "$NONLEADING_BASE" -p "$GITHUB_BASE"
)
if (validate_commit_range "$TARGET_OID" "$NONLEADING_MERGE" "$GITHUB_BASE") \
  >/dev/null 2>&1; then
  echo 'non-leading Forge merge checkpoint was accepted' >&2
  exit 1
fi

quiet_git "$DEST_REPO" reset --hard "$BASE"
if (replay_commit "$DEST_REPO" "$CHECKPOINT_SOURCE" "$TEST_ROOT/checkpoint-state" \
  "$GITHUB_BASE") >/dev/null 2>&1; then
  echo 'checkpoint replay over the wrong epic tree was accepted' >&2
  exit 1
fi
TAMPERED_CHECKPOINT=$(
  GIT_AUTHOR_NAME='Sync Agent' GIT_AUTHOR_EMAIL='sync@agents.duck' \
  GIT_AUTHOR_DATE='2026-07-03T01:02:03+00:00' GIT_COMMITTER_NAME='Sync Node' \
  GIT_COMMITTER_EMAIL='sync@nodes.duck' GIT_COMMITTER_DATE='2026-07-03T04:05:06+00:00' \
    git -c commit.gpgSign=false commit-tree "$CHECKPOINT_SOURCE^{tree}" \
      -p "$EPIC_PRE_CHECKPOINT" -p "$BASE" <"$checkpoint_message"
)
if (verify_replayed_commit "$CHECKPOINT_SOURCE" "$TAMPERED_CHECKPOINT" \
  "$TEST_ROOT/checkpoint-state" 'tampered checkpoint' "$GITHUB_BASE") >/dev/null 2>&1; then
  echo 'checkpoint with a changed GitHub parent was accepted' >&2
  exit 1
fi
TAMPERED_CHECKPOINT_TREE=$(
  GIT_AUTHOR_NAME='Sync Agent' GIT_AUTHOR_EMAIL='sync@agents.duck' \
  GIT_AUTHOR_DATE='2026-07-03T01:02:03+00:00' GIT_COMMITTER_NAME='Sync Node' \
  GIT_COMMITTER_EMAIL='sync@nodes.duck' GIT_COMMITTER_DATE='2026-07-03T04:05:06+00:00' \
    git -c commit.gpgSign=false commit-tree "$EPIC_PRE_CHECKPOINT^{tree}" \
      -p "$EPIC_PRE_CHECKPOINT" -p "$GITHUB_BASE" <"$checkpoint_message"
)
if (verify_replayed_commit "$CHECKPOINT_SOURCE" "$TAMPERED_CHECKPOINT_TREE" \
  "$TEST_ROOT/checkpoint-state" 'tampered checkpoint tree' "$GITHUB_BASE") >/dev/null 2>&1; then
  echo 'checkpoint with a changed tree was accepted' >&2
  exit 1
fi
quiet_git "$DEST_REPO" reset --hard "$MIRROR_OID"

# The bounded marker owns only its JSON ledger; human-authored text on either
# side survives an append byte-for-byte. The recorded replay is then proven
# from commit contents rather than trusted from PR text.
printf 'Human-authored epic introduction.\n' >"$TEST_ROOT/epic-body"
printf '%s\t%s\n' "$SOURCE_OID" "$MIRROR_OID" >"$TEST_ROOT/epic-mappings"
node - "$TEST_ROOT/epic-entry" "$SOURCE_OID" "$MIRROR_OID" "$MERGE_OID" "$TARGET_OID" <<'NODE'
const fs = require("node:fs");
const [out, source, mirror, merge, target] = process.argv.slice(2);
fs.writeFileSync(out, JSON.stringify({
  repo: "ducktape", pr: 26, merge, source, target, commits: [{source, mirror}],
}));
NODE
append_epic_ledger "$TEST_ROOT/epic-body" "$TEST_ROOT/epic-entry" "$TEST_ROOT/epic-body-once"
grep -F 'Human-authored epic introduction.' "$TEST_ROOT/epic-body-once" >/dev/null
parse_epic_ledger "$TEST_ROOT/epic-body-once" "$TEST_ROOT/epic-records"
grep -F $'E\tducktape\t26\t' "$TEST_ROOT/epic-records" >/dev/null
grep -F $'C\t'"$SOURCE_OID"$'\t'"$MIRROR_OID" "$TEST_ROOT/epic-records" >/dev/null
verify_replayed_commit "$SOURCE_OID" "$MIRROR_OID" "$TEST_ROOT" 'epic test'

# Every ledger entry is re-bound to its canonical merged Forge PR, not merely
# to valid Git objects. A GitHub body edit cannot relabel old commits or alter
# an older JSON entry's merge metadata while keeping its commit mappings.
FORGE_REPO=ducktape
FORGE_DEV=$(git rev-parse refs/mirror-test/forge-dev)
node - "$TEST_ROOT/canonical-item-26" "$MERGE_OID" <<'NODE'
const fs = require("node:fs");
const [out, merge] = process.argv.slice(2);
fs.writeFileSync(out, JSON.stringify({item: {
  number: 26, kind: "pr", state: "merged", target_branch: "dev", merge_oid: merge,
}}));
NODE
query_named_item() {
  local repo=$1 number=$2 out=$3
  if [ "$repo" = ducktape ] && [ "$number" = 26 ]; then
    cp "$TEST_ROOT/canonical-item-26" "$out"
  else
    printf '{"item":null}\n' >"$out"
  fi
}
read -r kind repo number merge source target < <(head -1 "$TEST_ROOT/epic-records")
[ "$kind" = E ]
validate_epic_item "$repo" "$number" "$merge" "$source" "$target" \
  "$FORGE_DEV" "$GITHUB_BASE" "$TEST_ROOT/epic-item-valid"
[ "${VALIDATED_EPIC_COMMITS[*]}" = "$SOURCE_OID" ]
SOURCE_COMMITS=("$SOURCE_OID")

node - "$TEST_ROOT/epic-body-once" "$TEST_ROOT/epic-body-wrong-repo" \
  "$TEST_ROOT/epic-body-wrong-pr" "$TEST_ROOT/epic-body-wrong-merge" "$BASE" <<'NODE'
const fs = require("node:fs");
const [input, wrongRepo, wrongPr, wrongMerge, replacementMerge] = process.argv.slice(2);
const body = fs.readFileSync(input, "utf8");
fs.writeFileSync(wrongRepo, body.replace('"repo": "ducktape"', '"repo": "other"'));
fs.writeFileSync(wrongPr, body.replace('"pr": 26', '"pr": 27'));
fs.writeFileSync(wrongMerge, body.replace(/"merge": "[0-9a-f]{40}"/, `"merge": "${replacementMerge}"`));
NODE
for tampered in wrong-repo wrong-pr wrong-merge; do
  parse_epic_ledger "$TEST_ROOT/epic-body-$tampered" "$TEST_ROOT/records-$tampered"
  read -r kind repo number merge source target < <(head -1 "$TEST_ROOT/records-$tampered")
  if (validate_epic_item "$repo" "$number" "$merge" "$source" "$target" \
    "$FORGE_DEV" "$GITHUB_BASE" "$TEST_ROOT/item-$tampered") >/dev/null 2>&1; then
    echo "tampered prior JSON epic entry was accepted: $tampered" >&2
    exit 1
  fi
done
SOURCE_COMMITS=("$SOURCE_OID")

# PR #598 was deployed with literal backslash-n separators throughout its
# body, not newline bytes. Keep the exact deployed shape as the fixture, parse
# its redundant pair strictly, and preserve every byte outside the markers.
node - "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-598-prefix" <<'NODE'
const fs = require("node:fs");
const [bodyPath, prefixPath] = process.argv.slice(2);
const body = String.raw`This draft accumulates independently reviewed Ducktape Agent-system improvements before one merge into dev.\n\n## Current ledger\n\n<!-- forge-epic-provenance:v1 -->\n<!-- forge-provenance:ducktape#117 merge=d688a1104daae08c38ac1e5938c6d7737d299314 source=20d9b9efd96d3a0741c8430b13a2981d1da1f66d target=cfd1bf1ad8e37afc32a07301a20ed580aa4bb227 -->\n- ducktape#117 — 20d9b9efd96d3a0741c8430b13a2981d1da1f66d (Forge merge d688a1104daae08c38ac1e5938c6d7737d299314)\n<!-- /forge-epic-provenance -->\n\n### Forge PR #117 — Stage bounded Librarian call contracts behind the re-genesis fence\n\n- Forge merge: d688a1104daae08c38ac1e5938c6d7737d299314\n- Forge source: 20d9b9efd96d3a0741c8430b13a2981d1da1f66d\n- GitHub replay tip: 9690bc80b1fe08232d69cd3c5bc5466baaaaa622\n- Clean review: two P1 findings addressed; follow-up found no P0/P1 and high merge confidence\n- Gates: Runs and MCP tests, crate clippy, no-default-features, wasm32 check, and 252 bounded Podman tests at 2 CPU / 4 GiB / swap 0\n- Version safety: no WASM module version, current_version, or upgrade schedule changed\n\n## Operating rule\n\n- Forge keeps one atomic issue and PR per change for Agent execution, audit, and rollback.\n- This GitHub PR remains draft while verified Forge changes are appended with original messages and provenance.\n- The append mechanism is generic to an epic branch and slug; it must not special-case Librarian work or issue numbers.\n- Every slice receives a clean-context review and bounded verification.\n- Testing may compile or compatibility-check WASM, but must never bump module versions, schedule activation, or force an upgrade.\n- WASM version changes require a separate, explicitly requested release task.\n- At milestone freeze, review the full diff against current dev, keep history intact, mark ready, and merge once with a merge commit.\n- Do not squash, rebase, or add GitHub issue-closing keywords.`;
const marker = "<!-- forge-epic-provenance:v1 -->";
fs.writeFileSync(bodyPath, body);
fs.writeFileSync(prefixPath, body.slice(0, body.indexOf(marker)));
NODE
parse_epic_ledger "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-598-records"
grep -F $'L\tducktape\t117\td688a1104daae08c38ac1e5938c6d7737d299314' \
  "$TEST_ROOT/epic-598-records" >/dev/null
node - "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-598-mixed" <<'NODE'
const fs = require("node:fs");
const [input, output] = process.argv.slice(2);
const body = fs.readFileSync(input, "utf8");
fs.writeFileSync(output, body.replace(String.raw`\n- ducktape#117`, "\n- ducktape#117"));
NODE
parse_epic_ledger "$TEST_ROOT/epic-598-mixed" "$TEST_ROOT/epic-598-mixed-records"
cmp "$TEST_ROOT/epic-598-records" "$TEST_ROOT/epic-598-mixed-records"
node - "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-598-mismatch" <<'NODE'
const fs = require("node:fs");
const [input, output] = process.argv.slice(2);
const body = fs.readFileSync(input, "utf8").replace(
  "(Forge merge d688a1104daae08c38ac1e5938c6d7737d299314)",
  "(Forge merge 0000000000000000000000000000000000000000)",
);
fs.writeFileSync(output, body);
NODE
if (parse_epic_ledger "$TEST_ROOT/epic-598-mismatch" \
  "$TEST_ROOT/epic-598-mismatch-records") >/dev/null 2>&1; then
  echo 'disagreeing legacy provenance comment and bullet were accepted' >&2
  exit 1
fi
cat >"$TEST_ROOT/epic-598-resolved" <<'EOF'
E	ducktape	117	d688a1104daae08c38ac1e5938c6d7737d299314	20d9b9efd96d3a0741c8430b13a2981d1da1f66d	cfd1bf1ad8e37afc32a07301a20ed580aa4bb227
C	55e431cbfa4986451bfb4813a1ca81206541c913	822c1713ea404ad801003716cee6c0eae9ebaa47
C	20d9b9efd96d3a0741c8430b13a2981d1da1f66d	9690bc80b1fe08232d69cd3c5bc5466baaaaa622
EOF
append_epic_ledger "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-entry" \
  "$TEST_ROOT/epic-598-migrated" "$TEST_ROOT/epic-598-resolved"
cmp "$TEST_ROOT/epic-598-prefix" \
  <(head -c "$(wc -c <"$TEST_ROOT/epic-598-prefix")" "$TEST_ROOT/epic-598-migrated")
node - "$TEST_ROOT/epic-598-body" "$TEST_ROOT/epic-598-migrated" <<'NODE'
const fs = require("node:fs");
const [beforePath, afterPath] = process.argv.slice(2);
const before = fs.readFileSync(beforePath, "utf8");
const after = fs.readFileSync(afterPath, "utf8");
const oldEnd = "<!-- /forge-epic-provenance -->";
const newEnd = "<!-- forge-epic-provenance:end -->";
const oldSuffix = before.slice(before.indexOf(oldEnd) + oldEnd.length);
const newSuffix = after.slice(after.indexOf(newEnd) + newEnd.length);
if (oldSuffix !== newSuffix) throw new Error("legacy migration changed human-authored suffix bytes");
NODE
grep -F '### Forge PR #117 — Stage bounded Librarian call contracts behind the re-genesis fence' \
  "$TEST_ROOT/epic-598-migrated" >/dev/null
[ "$(grep -Fc 'forge-epic-provenance:v1' "$TEST_ROOT/epic-598-migrated")" -eq 0 ]
[ "$(grep -Fc 'forge-epic-provenance:start' "$TEST_ROOT/epic-598-migrated")" -eq 1 ]
parse_epic_ledger "$TEST_ROOT/epic-598-migrated" "$TEST_ROOT/epic-598-migrated-records"
grep -F $'E\tducktape\t117\t' "$TEST_ROOT/epic-598-migrated-records" >/dev/null
grep -F $'E\tducktape\t26\t' "$TEST_ROOT/epic-598-migrated-records" >/dev/null

# Repeating an already-ledgered Forge provenance is detected instead of
# creating a second entry. Main treats the verified existing entry as a no-op.
if (append_epic_ledger "$TEST_ROOT/epic-body-once" "$TEST_ROOT/epic-entry" \
  "$TEST_ROOT/epic-body-twice") >/dev/null 2>&1; then
  echo 'duplicate epic provenance was appended' >&2
  exit 1
fi
[ "$(grep -Fc '"pr": 26' "$TEST_ROOT/epic-body-once")" -eq 1 ]

# A partial-success recovery is safe only when the branch commit has the exact
# source message, identities, timestamps, and patch. A mismatched branch
# commit is refused even if PR text claims otherwise.
printf 'Human-authored epic introduction.\n' >"$TEST_ROOT/epic-body-empty"
parse_epic_ledger "$TEST_ROOT/epic-body-empty" "$TEST_ROOT/epic-empty-records"
[ ! -s "$TEST_ROOT/epic-empty-records" ]
verify_replayed_commit "$SOURCE_OID" "$MIRROR_OID" "$TEST_ROOT" 'partial-success recovery'
if (verify_replayed_commit "$SOURCE_OID" "$BASE" "$TEST_ROOT" \
  'partial-success recovery') >/dev/null 2>&1; then
  echo 'mismatched partial-success commit was accepted' >&2
  exit 1
fi

# Exact semantic replay verification must reject whitespace differences even
# when message, author, committer, and times match.
quiet_git "$DEST_REPO" reset --hard "$MIRROR_OID"
printf 'x\n' >"$DEST_REPO/patch-exact.txt"
quiet_git "$DEST_REPO" add patch-exact.txt
quiet_git "$DEST_REPO" commit -m 'patch comparison base'
PATCH_EXACT_BASE=$(git rev-parse HEAD)
printf 'a b\n' >"$DEST_REPO/patch-exact.txt"
quiet_git "$DEST_REPO" add patch-exact.txt
GIT_AUTHOR_NAME='Patch Author' GIT_AUTHOR_EMAIL='patch@example.com' \
GIT_AUTHOR_DATE='2026-07-02T01:02:03+00:00' GIT_COMMITTER_NAME='Patch Committer' \
GIT_COMMITTER_EMAIL='patch-committer@example.com' GIT_COMMITTER_DATE='2026-07-02T04:05:06+00:00' \
  git -C "$DEST_REPO" commit -m 'preserve exact whitespace' >/dev/null
PATCH_EXACT_SOURCE=$(git rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$PATCH_EXACT_BASE"
printf 'ab\n' >"$DEST_REPO/patch-exact.txt"
quiet_git "$DEST_REPO" add patch-exact.txt
GIT_AUTHOR_NAME='Patch Author' GIT_AUTHOR_EMAIL='patch@example.com' \
GIT_AUTHOR_DATE='2026-07-02T01:02:03+00:00' GIT_COMMITTER_NAME='Patch Committer' \
GIT_COMMITTER_EMAIL='patch-committer@example.com' GIT_COMMITTER_DATE='2026-07-02T04:05:06+00:00' \
  git -C "$DEST_REPO" commit -m 'preserve exact whitespace' >/dev/null
PATCH_EXACT_COLLISION=$(git rev-parse HEAD)
if (verify_replayed_commit "$PATCH_EXACT_SOURCE" "$PATCH_EXACT_COLLISION" "$TEST_ROOT" \
  'whitespace collision') >/dev/null 2>&1; then
  echo 'whitespace-different replay was accepted' >&2
  exit 1
fi

# Hunk hashes can collide when identical context occurs twice. The semantic
# proof must reject a commit that changes the wrong occurrence even with exact
# provenance, and replay_commit must not accept a relocated cherry-pick.
quiet_git "$DEST_REPO" reset --hard "$MIRROR_OID"
printf 'a\nb\nc\nold\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\nf\n' \
  >"$DEST_REPO/replay-repeated.txt"
quiet_git "$DEST_REPO" add replay-repeated.txt
quiet_git "$DEST_REPO" commit -m 'semantic replay repeated base'
REPLAY_REPEATED_BASE=$(git rev-parse HEAD)
printf 'a\nb\nc\nforge\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\nf\n' \
  >"$DEST_REPO/replay-repeated.txt"
quiet_git "$DEST_REPO" add replay-repeated.txt
GIT_AUTHOR_NAME='Patch Author' GIT_AUTHOR_EMAIL='patch@example.com' \
GIT_AUTHOR_DATE='2026-07-02T01:02:03+00:00' GIT_COMMITTER_NAME='Patch Committer' \
GIT_COMMITTER_EMAIL='patch-committer@example.com' GIT_COMMITTER_DATE='2026-07-02T04:05:06+00:00' \
  git -C "$DEST_REPO" commit -m 'preserve exact repeated location' >/dev/null
REPLAY_REPEATED_SOURCE=$(git rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$REPLAY_REPEATED_BASE"
printf 'a\nb\nc\nold\nd\ne\nf\nmiddle\na\nb\nc\nforge\nd\ne\nf\n' \
  >"$DEST_REPO/replay-repeated.txt"
quiet_git "$DEST_REPO" add replay-repeated.txt
GIT_AUTHOR_NAME='Patch Author' GIT_AUTHOR_EMAIL='patch@example.com' \
GIT_AUTHOR_DATE='2026-07-02T01:02:03+00:00' GIT_COMMITTER_NAME='Patch Committer' \
GIT_COMMITTER_EMAIL='patch-committer@example.com' GIT_COMMITTER_DATE='2026-07-02T04:05:06+00:00' \
  git -C "$DEST_REPO" commit -m 'preserve exact repeated location' >/dev/null
REPLAY_REPEATED_WRONG=$(git rev-parse HEAD)
if (verify_replayed_commit "$REPLAY_REPEATED_SOURCE" "$REPLAY_REPEATED_WRONG" "$TEST_ROOT" \
  'repeated-context relocation') >/dev/null 2>&1; then
  echo 'change to the wrong repeated occurrence was accepted' >&2
  exit 1
fi
quiet_git "$DEST_REPO" reset --hard "$REPLAY_REPEATED_BASE"
printf 'a\nb\nc\ngithub\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\nf\n' \
  >"$DEST_REPO/replay-repeated.txt"
quiet_git "$DEST_REPO" add replay-repeated.txt
quiet_git "$DEST_REPO" commit -m 'GitHub changes the source occurrence'
mkdir "$TEST_ROOT/relocated-state"
if (replay_commit "$DEST_REPO" "$REPLAY_REPEATED_SOURCE" "$TEST_ROOT/relocated-state") \
  >/dev/null 2>&1; then
  echo 'relocated cherry-pick was accepted by initial replay' >&2
  exit 1
fi

# A non-overlapping GitHub edit in the same file remains valid.
quiet_git "$DEST_REPO" reset --hard "$REPLAY_REPEATED_BASE"
printf 'a\nb\nc\nold\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\ngithub\n' \
  >"$DEST_REPO/replay-repeated.txt"
quiet_git "$DEST_REPO" add replay-repeated.txt
quiet_git "$DEST_REPO" commit -m 'GitHub changes an unrelated same-file line'
mkdir "$TEST_ROOT/unrelated-state"
replay_commit "$DEST_REPO" "$REPLAY_REPEATED_SOURCE" "$TEST_ROOT/unrelated-state"
[ "$(sed -n '4p' "$DEST_REPO/replay-repeated.txt")" = forge ]
[ "$(sed -n '15p' "$DEST_REPO/replay-repeated.txt")" = github ]

# Git's three-tree semantics also preserve exact binary content, renames, and
# executable modes while replaying over unrelated GitHub history.
quiet_git "$DEST_REPO" reset --hard "$MIRROR_OID"
printf '\000base\377\n' >"$DEST_REPO/replay.bin"
printf 'rename me\n' >"$DEST_REPO/rename-old.txt"
printf '#!/bin/sh\nexit 0\n' >"$DEST_REPO/mode.sh"
chmod 644 "$DEST_REPO/mode.sh"
quiet_git "$DEST_REPO" add replay.bin rename-old.txt mode.sh
quiet_git "$DEST_REPO" commit -m 'semantic replay special-file base'
SPECIAL_BASE=$(git rev-parse HEAD)
printf '\000forge\376\n' >"$DEST_REPO/replay.bin"
quiet_git "$DEST_REPO" mv rename-old.txt rename-new.txt
chmod 755 "$DEST_REPO/mode.sh"
quiet_git "$DEST_REPO" add replay.bin mode.sh
quiet_git "$DEST_REPO" commit -m 'change binary rename and mode'
SPECIAL_SOURCE=$(git rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$SPECIAL_BASE"
printf 'github side change\n' >"$DEST_REPO/github-special.txt"
quiet_git "$DEST_REPO" add github-special.txt
quiet_git "$DEST_REPO" commit -m 'GitHub advances beside special files'
mkdir "$TEST_ROOT/special-state"
replay_commit "$DEST_REPO" "$SPECIAL_SOURCE" "$TEST_ROOT/special-state"
cmp "$DEST_REPO/replay.bin" <(printf '\000forge\376\n')
[ -f "$DEST_REPO/rename-new.txt" ] && [ ! -e "$DEST_REPO/rename-old.txt" ]
[ "$(git -C "$DEST_REPO" ls-tree HEAD mode.sh | awk '{print $1}')" = 100755 ]
quiet_git "$DEST_REPO" reset --hard "$MIRROR_OID"

# Existing epic PR bodies are read-only. New provenance is appended as a
# visible comment, and only byte-identical duplicate records are deduplicated.
GH_REPO='example/ducktape'
MIRROR_BRANCH='improvement/platform-quality'
RUN_DIR="$TEST_ROOT/state"
write_epic_comment "$TEST_ROOT/epic-entry" 'Fixes #49 <unsafe>' "$TEST_ROOT/epic-comment"
grep -F 'addresses Forge ducktape item 49' "$TEST_ROOT/epic-comment" >/dev/null
grep -F '&lt;unsafe&gt;' "$TEST_ROOT/epic-comment" >/dev/null
if grep -F 'Fixes #49' "$TEST_ROOT/epic-comment" >/dev/null; then
  echo 'closing keyword survived in an epic provenance comment' >&2
  exit 1
fi
node - "$TEST_ROOT/epic-comment" "$TEST_ROOT/epic-comments" \
  "$TEST_ROOT/epic-comments-duplicate" "$TEST_ROOT/epic-comments-conflict" "$BASE" <<'NODE'
const fs = require("node:fs");
const [commentPath, commentsPath, duplicatePath, conflictPath, replacement] = process.argv.slice(2);
const body = fs.readFileSync(commentPath, "utf8");
const comment = {id: 1, body, user: {login: "epic-author"}};
fs.writeFileSync(commentsPath, JSON.stringify([[
  {id: 0, body: "human comment", user: {login: "reader"}},
  {id: 4, body, user: {login: "untrusted"}},
  comment,
]]));
fs.writeFileSync(duplicatePath, JSON.stringify([[comment, {
  id: 2, body, user: {login: "epic-author"},
}]]));
fs.writeFileSync(conflictPath, JSON.stringify([[comment, {
  id: 3, body: body.replace(/"source": "[0-9a-f]{40}"/, `"source": "${replacement}"`),
  user: {login: "epic-author"},
}]]));
NODE
parse_epic_comments "$TEST_ROOT/epic-comments" "$TEST_ROOT/epic-comment-records" epic-author
grep -F $'E\tducktape\t26\t' "$TEST_ROOT/epic-comment-records" >/dev/null
grep -F $'C\t'"$SOURCE_OID"$'\t'"$MIRROR_OID" "$TEST_ROOT/epic-comment-records" >/dev/null
parse_epic_comments "$TEST_ROOT/epic-comments-duplicate" \
  "$TEST_ROOT/epic-comment-records-duplicate" epic-author
cmp "$TEST_ROOT/epic-comment-records" "$TEST_ROOT/epic-comment-records-duplicate"
if (parse_epic_comments "$TEST_ROOT/epic-comments-conflict" \
  "$TEST_ROOT/epic-comment-records-conflict" epic-author) >/dev/null 2>&1; then
  echo 'conflicting duplicate epic provenance comments were accepted' >&2
  exit 1
fi
require_epic_author epic-author epic-author
if (require_epic_author epic-author another-user) >/dev/null 2>&1; then
  echo 'a different GitHub login passed the epic-author gate' >&2
  exit 1
fi
MOCK_POST_FAIL=0
gh() {
  if [[ " $* " == *' --paginate --slurp '* &&
        " $* " == *'/issues/598/comments?per_page=100&sort=created&direction=asc '* ]]; then
    cat "$TEST_ROOT/epic-comments"
    return
  fi
  [[ " $* " == *' --method POST '* && " $* " == *'/issues/598/comments '* ]] || return 97
  [ "$MOCK_POST_FAIL" -eq 0 ] || return 1
  local args=("$@") input=""
  for ((i = 0; i < ${#args[@]}; i += 1)); do
    if [ "${args[$i]}" = --input ]; then input=${args[$((i + 1))]}; fi
  done
  node - "$input" <<'NODE'
const fs = require("node:fs");
process.stdout.write(JSON.stringify({id: 4, user: {login: "epic-author"},
  ...JSON.parse(fs.readFileSync(process.argv[2], "utf8"))}));
NODE
}
query_epic_comments 598 "$TEST_ROOT/epic-comments-queried"
cmp "$TEST_ROOT/epic-comments" "$TEST_ROOT/epic-comments-queried"
post_epic_comment 598 "$TEST_ROOT/epic-comment" "$TEST_ROOT/epic-comment-response" \
  "$TEST_ROOT/state"
MOCK_POST_FAIL=1
if (post_epic_comment 598 "$TEST_ROOT/epic-comment" \
  "$TEST_ROOT/epic-comment-response-lost" "$TEST_ROOT/state") >/dev/null 2>&1; then
  echo 'an uncertain epic-comment POST was treated as success' >&2
  exit 1
fi
# The next run rereads the server; an exact duplicate from an accepted POST
# with a lost response is already proven idempotent by the duplicate fixture.
parse_epic_comments "$TEST_ROOT/epic-comments-duplicate" \
  "$TEST_ROOT/epic-comment-records-after-lost-post" epic-author
cmp "$TEST_ROOT/epic-comment-records" "$TEST_ROOT/epic-comment-records-after-lost-post"
unset -f gh

assert_shared_dev_base "$TARGET_OID" "$GITHUB_BASE"
assert_shared_dev_base "$MERGE_OID" "$MIRROR_OID"
assert_epic_shared_dev_base "$TARGET_OID" "$BASE" "$TARGET_OID"
[ "$EPIC_BASE_REPRESENTATION" = epic ]
assert_epic_shared_dev_base "$TARGET_OID" "$GITHUB_BASE" "$BASE"
[ "$EPIC_BASE_REPRESENTATION" = origin ]
if (assert_epic_shared_dev_base "$MERGE_OID" "$GITHUB_BASE" "$SOURCE_OID") \
  >/dev/null 2>&1; then
  echo 'partial target deltas split across origin and epic were accepted' >&2
  exit 1
fi
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

# An epic can remain forked before a baseline later represented in GitHub dev.
# Validate against current dev, then replay only the later Forge delta on the
# stale epic tip; a target delta absent from current dev must still fail closed.
STALE_EPIC_TIP=$(printf 'stale epic slice\n' | git commit-tree "$BASE^{tree}" -p "$BASE")
quiet_git "$DEST_REPO" reset --hard "$BASE"
printf 'represented baseline\n' >"$DEST_REPO/later-baseline.txt"
quiet_git "$DEST_REPO" add later-baseline.txt
quiet_git "$DEST_REPO" commit -m 'GitHub receives later baseline'
printf 'current GitHub advance\n' >"$DEST_REPO/current-github.txt"
quiet_git "$DEST_REPO" add current-github.txt
quiet_git "$DEST_REPO" commit -m 'GitHub advances after represented baseline'
CURRENT_GITHUB_DEV=$(git -C "$DEST_REPO" rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$BASE"
printf 'represented baseline\n' >"$DEST_REPO/later-baseline.txt"
quiet_git "$DEST_REPO" add later-baseline.txt
quiet_git "$DEST_REPO" commit -m 'Forge receives later baseline'
LATER_FORGE_TARGET=$(git -C "$DEST_REPO" rev-parse HEAD)
printf 'later Forge delta\n' >"$DEST_REPO/later-forge.txt"
quiet_git "$DEST_REPO" add later-forge.txt
quiet_git "$DEST_REPO" commit -m 'Later Forge delta'
LATER_FORGE_SOURCE=$(git -C "$DEST_REPO" rev-parse HEAD)
assert_shared_dev_base "$LATER_FORGE_TARGET" "$CURRENT_GITHUB_DEV"
assert_epic_shared_dev_base "$LATER_FORGE_TARGET" "$CURRENT_GITHUB_DEV" "$STALE_EPIC_TIP"
[ "$EPIC_BASE_REPRESENTATION" = origin ]
validate_commit_range "$LATER_FORGE_TARGET" "$LATER_FORGE_SOURCE"
quiet_git "$DEST_REPO" reset --hard "$STALE_EPIC_TIP"
mkdir "$TEST_ROOT/stale-epic-state"
replay_commit "$DEST_REPO" "$LATER_FORGE_SOURCE" "$TEST_ROOT/stale-epic-state"
[ "$(git -C "$DEST_REPO" rev-parse HEAD^1)" = "$STALE_EPIC_TIP" ]
[ "$(cat "$DEST_REPO/later-forge.txt")" = 'later Forge delta' ]
quiet_git "$DEST_REPO" reset --hard "$LATER_FORGE_TARGET"
printf 'missing from GitHub dev\n' >"$DEST_REPO/forge-target-only.txt"
quiet_git "$DEST_REPO" add forge-target-only.txt
quiet_git "$DEST_REPO" commit -m 'Forge target delta missing from GitHub'
MISSING_FORGE_TARGET=$(git -C "$DEST_REPO" rev-parse HEAD)
if (assert_epic_shared_dev_base "$MISSING_FORGE_TARGET" "$CURRENT_GITHUB_DEV" \
  "$STALE_EPIC_TIP") >/dev/null 2>&1; then
  echo 'a Forge target delta missing from current GitHub dev and epic was accepted' >&2
  exit 1
fi
quiet_git "$DEST_REPO" reset --hard "$GITHUB_BASE"

# GitHub may add an unrelated change elsewhere in a file that already carries
# the Forge delta. The three-tree proof accepts it without requiring whole-file
# identity.
quiet_git "$DEST_REPO" reset --hard "$BASE"
printf 'alpha\nold\nomega\nextra-old\n' >"$DEST_REPO/represented.txt"
quiet_git "$DEST_REPO" add represented.txt
quiet_git "$DEST_REPO" commit -m 'represented fixture base'
REPRESENTED_BASE=$(git -C "$DEST_REPO" rev-parse HEAD)
printf 'alpha\nforge\nomega\nextra-old\n' >"$DEST_REPO/represented.txt"
quiet_git "$DEST_REPO" add represented.txt
quiet_git "$DEST_REPO" commit -m 'Forge changes one hunk'
REPRESENTED_FORGE=$(git -C "$DEST_REPO" rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$REPRESENTED_BASE"
printf 'alpha\nforge\nomega\ngithub\n' >"$DEST_REPO/represented.txt"
quiet_git "$DEST_REPO" add represented.txt
quiet_git "$DEST_REPO" commit -m 'GitHub keeps Forge and changes another hunk'
REPRESENTED_GITHUB=$(git -C "$DEST_REPO" rev-parse HEAD)
assert_shared_dev_base "$REPRESENTED_FORGE" "$REPRESENTED_GITHUB"

# Identical patch context at another location must not let a missing Forge
# change relocate and pass. GitHub changed only the second repeated block.
quiet_git "$DEST_REPO" reset --hard "$BASE"
printf 'a\nb\nc\nold\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\nf\n' \
  >"$DEST_REPO/repeated.txt"
quiet_git "$DEST_REPO" add repeated.txt
quiet_git "$DEST_REPO" commit -m 'repeated context base'
REPEATED_BASE=$(git -C "$DEST_REPO" rev-parse HEAD)
printf 'a\nb\nc\nforge\nd\ne\nf\nmiddle\na\nb\nc\nold\nd\ne\nf\n' \
  >"$DEST_REPO/repeated.txt"
quiet_git "$DEST_REPO" add repeated.txt
quiet_git "$DEST_REPO" commit -m 'Forge changes the first repeated block'
REPEATED_FORGE=$(git -C "$DEST_REPO" rev-parse HEAD)
quiet_git "$DEST_REPO" reset --hard "$REPEATED_BASE"
printf 'a\nb\nc\nold\nd\ne\nf\nmiddle\na\nb\nc\nforge\nd\ne\nf\n' \
  >"$DEST_REPO/repeated.txt"
quiet_git "$DEST_REPO" add repeated.txt
quiet_git "$DEST_REPO" commit -m 'GitHub changes only the second repeated block'
REPEATED_GITHUB=$(git -C "$DEST_REPO" rev-parse HEAD)
if (assert_shared_dev_base "$REPEATED_FORGE" "$REPEATED_GITHUB") >/dev/null 2>&1; then
  echo 'a Forge hunk missing at its original location was accepted' >&2
  exit 1
fi
quiet_git "$DEST_REPO" reset --hard "$GITHUB_BASE"

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
grep -F 'assert_shared_dev_base "$forge_target" "$origin_oid"' \
  "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null
grep -F 'git worktree add --detach "$MIRROR_WORKTREE" "$replay_base"' \
  "$REPO_ROOT/ops/mirror-forge-pr.sh" >/dev/null
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
