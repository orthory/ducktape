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

query_named_item() {
  local repo=$1 number=$2 out=$3
  [[ "$repo" =~ ^[a-z0-9._-]{1,64}$ ]] || die "Forge item query has an unsafe repo slug"
  [[ "$number" =~ ^[1-9][0-9]*$ ]] || die "Forge item query has an invalid number"
  curl --fail --silent --show-error --max-time 10 \
    -X POST "$BASE_URL/v1/query" \
    -H 'content-type: application/json' \
    --data "{\"target\":\"forge\",\"query\":{\"get_item\":{\"repo\":\"$repo\",\"number\":$number}}}" \
    >"$out"
}

query_item() {
  query_named_item "$FORGE_REPO" "$PR_NUMBER" "$1"
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
if (item.target_branch !== "dev") fail("Forge PR target must be dev");
if (typeof item.source_branch !== "string" || !item.source_branch) fail("Forge PR has no source branch");
if (!/^[0-9a-f]{40}$/i.test(item.merge_oid || "")) fail("Forge PR has no valid merge oid");
if (!/^[0-9a-f]{40}$/i.test(sourceOid)) fail("source commit must be 40 hex characters");
if (typeof item.title !== "string" || !item.title.trim()) fail("Forge PR title is empty");
if (/[\u0000-\u001f\u007f]/.test(item.title)) fail("Forge PR title must be one printable line");
fs.writeFileSync(fieldsPath, [item.source_branch, item.target_branch, item.merge_oid, item.title].join("\n") + "\n");
fs.writeFileSync(bodyPath, item.body || "");
NODE
}

validate_epic_item_reply() {
  local json=$1 repo=$2 number=$3 merge=$4
  node - "$json" "$repo" "$number" "$merge" <<'NODE'
const fs = require("node:fs");
const [jsonPath, repo, numberText, merge] = process.argv.slice(2);
const reply = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
const item = reply.item;
if (!item || item.number !== Number(numberText) || item.kind !== "pr" ||
    item.state !== "merged" || item.target_branch !== "dev" ||
    item.merge_oid !== merge) {
  throw new Error(`epic provenance does not match canonical Forge ${repo}#${numberText}`);
}
NODE
}

validate_epic_item() {
  local repo=$1 number=$2 merge=$3 source=$4 target=$5 forge_dev=$6 json=$7
  [ "$repo" = "$FORGE_REPO" ] ||
    die "epic provenance repo $repo does not match Forge mirror repo $FORGE_REPO"
  query_named_item "$repo" "$number" "$json" ||
    die "could not read canonical Forge $repo#$number"
  validate_epic_item_reply "$json" "$repo" "$number" "$merge" ||
    die "epic provenance does not match canonical Forge $repo#$number"
  git merge-base --is-ancestor "$merge" "$forge_dev" ||
    die "epic provenance merge $merge is not in canonical Forge dev"
  local canonical_target
  canonical_target=$(validate_merged_selection "$merge" "$source")
  [ "$canonical_target" = "$target" ] ||
    die "epic provenance target does not match canonical Forge $repo#$number"
  validate_commit_range "$target" "$source"
  VALIDATED_EPIC_COMMITS=("${SOURCE_COMMITS[@]}")
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

assert_shared_dev_base() {
  local forge_target=$1 github_dev=$2
  is_oid "$forge_target" && is_oid "$github_dev" || die "dev sync check contains an invalid oid"
  if git merge-base --is-ancestor "$forge_target" "$github_dev"; then
    return
  fi
  if git merge-base --is-ancestor "$github_dev" "$forge_target" &&
    git diff --quiet "$github_dev" "$forge_target"; then
    return
  fi
  local github_trees represented_base candidate_commit candidate_tree
  github_trees=$(git log --format=%T "$github_dev")
  represented_base=
  while read -r candidate_commit candidate_tree; do
    if grep -Fqx -- "$candidate_tree" <<<"$github_trees"; then
      represented_base=$candidate_commit
      break
    fi
  done < <(git log --topo-order --format='%H %T' "$forge_target")
  [ -n "$represented_base" ] ||
    die "Forge dev target $forge_target has no snapshot represented in GitHub dev $github_dev"
  if git diff --quiet "$represented_base" "$forge_target"; then
    return
  fi

  # GitHub may have advanced on the same files after an earlier Forge PR was
  # mirrored. Merge its current tree with the Forge target from the exact
  # shared snapshot. The Forge delta is already represented only when that
  # location-preserving three-tree merge is clean and changes nothing on GitHub.
  local merged_tree github_tree
  merged_tree=$(git merge-tree --write-tree --no-messages \
    --merge-base "$represented_base" "$github_dev" "$forge_target") ||
    die "Forge dev target $forge_target conflicts with GitHub dev $github_dev after their represented snapshot"
  github_tree=$(git rev-parse "$github_dev^{tree}")
  [ "$merged_tree" = "$github_tree" ] ||
    die "Forge dev target $forge_target contains changes missing from GitHub dev $github_dev"
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

  local source_raw="$state_dir/source-commit"
  local source_message="$state_dir/source-message"
  write_raw_message "$source" "$source_raw" "$source_message"

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

  verify_replayed_commit "$source" "$mirrored" "$state_dir" "replayed commit"
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

normalize_epic_branch() {
  local input=$1
  [ -n "$input" ] || die "epic identifier is empty"
  [[ "$input" != -* && "$input" != refs/* ]] || die "epic identifier is unsafe: $input"
  if [[ "$input" == */* ]]; then
    EPIC_BRANCH=$input
  else
    EPIC_BRANCH="improvement/$input"
  fi
  git check-ref-format --branch "$EPIC_BRANCH" >/dev/null 2>&1 ||
    die "epic identifier does not form a safe branch: $input"
  [ "$EPIC_BRANCH" != dev ] || die "epic branch must not be dev"
}

resolve_epic_history_base() {
  local github_dev=$1 epic_tip=$2 base
  is_oid "$github_dev" && is_oid "$epic_tip" || die "epic history contains an invalid oid"
  base=$(git merge-base "$github_dev" "$epic_tip") ||
    die "epic branch and current origin/dev have no shared history"
  is_oid "$base" || die "epic branch merge-base is invalid"
  git merge-base --is-ancestor "$base" "$github_dev" &&
    git merge-base --is-ancestor "$base" "$epic_tip" ||
    die "epic branch merge-base is not shared by both histories"
  printf '%s\n' "$base"
}

verify_replayed_commit() {
  local source=$1 mirrored=$2 state_dir=$3 label=$4
  is_oid "$source" && is_oid "$mirrored" || die "$label contains an invalid commit oid"
  git cat-file -e "$source^{commit}" 2>/dev/null || die "$label source commit $source is unavailable"
  git cat-file -e "$mirrored^{commit}" 2>/dev/null || die "$label mirrored commit $mirrored is unavailable"
  local source_message="$state_dir/verify-source-message" mirrored_message="$state_dir/verify-mirrored-message"
  local source_meta="$state_dir/verify-source-meta" mirrored_meta="$state_dir/verify-mirrored-meta"
  write_raw_message "$source" "$state_dir/verify-source-raw" "$source_message"
  write_raw_message "$mirrored" "$state_dir/verify-mirrored-raw" "$mirrored_message"
  commit_identity_headers "$source" >"$source_meta"
  commit_identity_headers "$mirrored" >"$mirrored_meta"
  cmp -s "$source_message" "$mirrored_message" || die "$label changed the raw message for $source"
  cmp -s "$source_meta" "$mirrored_meta" || die "$label changed author or committer provenance for $source"

  local source_parent_line mirrored_parent_line
  source_parent_line=$(git show -s --format=%P "$source")
  mirrored_parent_line=$(git show -s --format=%P "$mirrored")
  local -a source_parents=() mirrored_parents=()
  read -r -a source_parents <<<"$source_parent_line"
  read -r -a mirrored_parents <<<"$mirrored_parent_line"
  [ "${#source_parents[@]}" -eq 1 ] || die "$label source commit $source is not a single-parent commit"
  [ "${#mirrored_parents[@]}" -eq 1 ] || die "$label mirrored commit $mirrored is not a single-parent commit"

  # Give Git the real source parent A as the merge base while representing the
  # mirrored parent tree B on a synthetic A child. Merging that child with S
  # applies exactly A..S over B, including path, mode, rename, and binary
  # semantics, while retaining unrelated GitHub changes already present in B.
  local synthetic result_tree mirrored_tree
  synthetic=$(
    printf 'synthetic replay verification parent\n' |
      GIT_AUTHOR_NAME='Forge replay verifier' GIT_AUTHOR_EMAIL='forge-replay@invalid' \
      GIT_AUTHOR_DATE='2000-01-01T00:00:00+00:00' \
      GIT_COMMITTER_NAME='Forge replay verifier' GIT_COMMITTER_EMAIL='forge-replay@invalid' \
      GIT_COMMITTER_DATE='2000-01-01T00:00:00+00:00' \
      git -c commit.gpgSign=false commit-tree "${mirrored_parents[0]}^{tree}" -p "${source_parents[0]}"
  ) || die "$label could not construct its semantic replay base"
  result_tree=$(git merge-tree --write-tree --no-messages \
    --merge-base "${source_parents[0]}" "$synthetic" "$source") ||
    die "$label source commit $source does not apply cleanly to the mirrored parent"
  is_oid "$result_tree" || die "$label semantic replay returned an invalid tree oid"
  mirrored_tree=$(git rev-parse "$mirrored^{tree}")
  [ "$result_tree" = "$mirrored_tree" ] || die "$label changed the semantic patch for $source"
}

parse_epic_ledger() {
  local body=$1 records=$2
  node - "$body" "$records" <<'NODE'
const fs = require("node:fs");
const [bodyPath, recordsPath] = process.argv.slice(2);
const body = fs.readFileSync(bodyPath, "utf8");
if (Buffer.byteLength(body) > 1024 * 1024) throw new Error("GitHub epic PR body exceeds 1 MiB");
const start = "<!-- forge-epic-provenance:start -->";
const end = "<!-- forge-epic-provenance:end -->";
const legacy = "<!-- forge-epic-provenance:v1 -->";
const legacyEnd = "<!-- /forge-epic-provenance -->";
const starts = body.split(start).length - 1;
const ends = body.split(end).length - 1;
const legacies = body.split(legacy).length - 1;
const legacyEnds = body.split(legacyEnd).length - 1;
if (starts !== ends || starts > 1 || legacies > 1 || legacyEnds > 1 ||
    (starts && (legacies || legacyEnds)) || (!legacies && legacyEnds)) {
  throw new Error("epic provenance markers are malformed or duplicated");
}
if (!starts && !legacies) {
  fs.writeFileSync(recordsPath, "");
  process.exit(0);
}
if (legacies && legacyEnds) {
  const a = body.indexOf(legacy) + legacy.length;
  const b = body.indexOf(legacyEnd, a);
  if (b < a) throw new Error("legacy epic provenance markers are out of order");
  // PR #598's first writer stored literal "\\n" separators. Interpret either
  // separator only inside this already-bounded legacy block; prefix and suffix
  // bytes remain opaque and are preserved exactly during migration.
  const lines = body.slice(a, b).split(/\r?\n|\\n/).map(line => line.trim()).filter(Boolean);
  if (!lines.length || lines.length % 2 || lines.length > 256) {
    throw new Error("legacy epic provenance block has an unsupported shape");
  }
  const comment = /^<!-- forge-provenance:([a-z0-9._-]{1,64})#([1-9][0-9]*) merge=([0-9a-f]{40}) source=([0-9a-f]{40}) target=([0-9a-f]{40}) -->$/;
  const bullet = /^- ([a-z0-9._-]{1,64})#([1-9][0-9]*) — ([0-9a-f]{40}) \(Forge merge ([0-9a-f]{40})\)$/;
  const records = [];
  const seen = new Set();
  for (let i = 0; i < lines.length; i += 2) {
    const provenance = lines[i].match(comment);
    const summary = lines[i + 1].match(bullet);
    if (!provenance || !summary || provenance[1] !== summary[1] ||
        provenance[2] !== summary[2] || provenance[3] !== summary[4] ||
        provenance[4] !== summary[3]) {
      throw new Error("legacy epic provenance comment and summary disagree");
    }
    const key = `${provenance[1]}#${provenance[2]}`;
    if (seen.has(key)) throw new Error(`legacy epic provenance duplicates ${key}`);
    seen.add(key);
    records.push(["L", ...provenance.slice(1)].join("\t"));
  }
  fs.writeFileSync(recordsPath, records.join("\n") + "\n");
  process.exit(0);
}
let raw;
if (starts) {
  const a = body.indexOf(start) + start.length;
  const b = body.indexOf(end, a);
  if (b < a) throw new Error("epic provenance markers are out of order");
  raw = body.slice(a, b).trim();
} else {
  raw = body.slice(body.indexOf(legacy) + legacy.length).trim();
  const fenced = raw.match(/^```(?:json)?\s*\n([\s\S]*?)\n```\s*$/);
  if (fenced) raw = fenced[1].trim();
}
let ledger;
try { ledger = JSON.parse(raw); }
catch { throw new Error("epic provenance ledger is not valid JSON"); }
if (Array.isArray(ledger)) ledger = {version: 1, entries: ledger};
if (ledger && !Array.isArray(ledger.entries) && ledger.repo) ledger = {version: 1, entries: [ledger]};
if (!ledger || ledger.version !== 1 || !Array.isArray(ledger.entries) || ledger.entries.length > 128) {
  throw new Error("epic provenance ledger has an unsupported shape");
}
const oid = value => typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
const seen = new Set();
const lines = [];
for (const entry of ledger.entries) {
  if (!entry || typeof entry.repo !== "string" || !/^[a-z0-9._-]{1,64}$/.test(entry.repo) ||
      !Number.isSafeInteger(entry.pr) || entry.pr < 1 || !oid(entry.merge) || !oid(entry.source) ||
      !oid(entry.target) || !Array.isArray(entry.commits) || !entry.commits.length) {
    throw new Error("epic provenance ledger contains an invalid entry");
  }
  const key = `${entry.repo}#${entry.pr}`;
  if (seen.has(key)) throw new Error(`epic provenance ledger duplicates ${key}`);
  seen.add(key);
  lines.push(["E", entry.repo, entry.pr, entry.merge, entry.source, entry.target].join("\t"));
  for (const commit of entry.commits) {
    if (!commit || !oid(commit.source) || !oid(commit.mirror)) {
      throw new Error(`epic provenance ledger contains an invalid commit for ${key}`);
    }
    lines.push(["C", commit.source, commit.mirror].join("\t"));
  }
}
fs.writeFileSync(recordsPath, lines.length ? lines.join("\n") + "\n" : "");
NODE
}

append_epic_ledger() {
  local body=$1 entry=$2 output=$3 records=${4:-}
  node - "$body" "$entry" "$output" "$records" <<'NODE'
const fs = require("node:fs");
const [bodyPath, entryPath, outputPath, recordsPath] = process.argv.slice(2);
let body = fs.readFileSync(bodyPath, "utf8");
const entry = JSON.parse(fs.readFileSync(entryPath, "utf8"));
const start = "<!-- forge-epic-provenance:start -->";
const end = "<!-- forge-epic-provenance:end -->";
const legacy = "<!-- forge-epic-provenance:v1 -->";
const legacyEnd = "<!-- /forge-epic-provenance -->";
const a = body.indexOf(start);
const legacyAt = body.indexOf(legacy);
let ledger = {version: 1, entries: []};
if (recordsPath) {
  const lines = fs.readFileSync(recordsPath, "utf8").trim().split("\n").filter(Boolean);
  const seen = new Set();
  let current = null;
  const oid = value => /^[0-9a-f]{40}$/.test(value || "");
  for (const line of lines) {
    const fields = line.split("\t");
    if (fields[0] === "E" && fields.length === 6) {
      const [, repo, prText, merge, source, target] = fields;
      const pr = Number(prText);
      const key = `${repo}#${prText}`;
      if (!/^[a-z0-9._-]{1,64}$/.test(repo) || !Number.isSafeInteger(pr) || pr < 1 ||
          !oid(merge) || !oid(source) || !oid(target) || seen.has(key)) {
        throw new Error("resolved epic provenance records are invalid");
      }
      seen.add(key);
      current = {repo, pr, merge, source, target, commits: []};
      ledger.entries.push(current);
    } else if (fields[0] === "C" && fields.length === 3 && current &&
               oid(fields[1]) && oid(fields[2])) {
      current.commits.push({source: fields[1], mirror: fields[2]});
    } else {
      throw new Error("resolved epic provenance records are invalid");
    }
  }
  if (ledger.entries.some(old => !old.commits.length)) {
    throw new Error("resolved epic provenance entry has no commits");
  }
}
let prefix = body;
let suffix = "";
if (a >= 0 && legacyAt >= 0) throw new Error("epic provenance markers changed while updating");
if (a >= 0) {
  const contentStart = a + start.length;
  const b = body.indexOf(end, contentStart);
  if (b < 0 || body.indexOf(start, contentStart) >= 0 || body.indexOf(end, b + end.length) >= 0) {
    throw new Error("epic provenance markers changed while updating");
  }
  if (!recordsPath) ledger = JSON.parse(body.slice(contentStart, b).trim());
  prefix = body.slice(0, a);
  suffix = body.slice(b + end.length);
} else if (legacyAt >= 0) {
  if (body.indexOf(legacy, legacyAt + legacy.length) >= 0) {
    throw new Error("legacy epic provenance marker is duplicated");
  }
  prefix = body.slice(0, legacyAt);
  const legacyEndAt = body.indexOf(legacyEnd, legacyAt + legacy.length);
  if (legacyEndAt >= 0) {
    if (body.indexOf(legacyEnd, legacyEndAt + legacyEnd.length) >= 0) {
      throw new Error("legacy epic provenance end marker is duplicated");
    }
    suffix = body.slice(legacyEndAt + legacyEnd.length);
    if (!recordsPath) {
      throw new Error("legacy epic provenance requires verified commit mappings");
    }
  } else {
    let raw = body.slice(legacyAt + legacy.length).trim();
    const fenced = raw.match(/^```(?:json)?\s*\n([\s\S]*?)\n```\s*$/);
    if (fenced) raw = fenced[1].trim();
    if (!recordsPath) {
      ledger = JSON.parse(raw);
      if (Array.isArray(ledger)) ledger = {version: 1, entries: ledger};
      if (ledger && !Array.isArray(ledger.entries) && ledger.repo) {
        ledger = {version: 1, entries: [ledger]};
      }
    }
  }
}
if (ledger.entries.some(old => old.repo === entry.repo && old.pr === entry.pr)) {
  throw new Error(`epic provenance already contains ${entry.repo}#${entry.pr}`);
}
ledger.entries.push(entry);
const block = `${start}\n${JSON.stringify(ledger, null, 2)}\n${end}`;
if (a >= 0 || legacyAt >= 0) {
  body = `${prefix}${block}${suffix}`;
} else {
  const separator = !prefix ? "" : prefix.endsWith("\n") ? "\n" : "\n\n";
  body = `${prefix}${separator}${block}`;
}

if (Buffer.byteLength(body) > 1024 * 1024) throw new Error("updated GitHub epic PR body exceeds 1 MiB");
fs.writeFileSync(outputPath, body);
NODE
}

query_epic_comments() {
  local number=$1 out=$2
  gh api --paginate --slurp \
    "repos/$GH_REPO/issues/$number/comments?per_page=100&sort=created&direction=asc" >"$out" \
    2>"$RUN_DIR/gh.err" ||
    die "could not read epic PR provenance comments: $(tr '\n' ' ' <"$RUN_DIR/gh.err")"
}

require_epic_author() {
  local expected=$1 actual=$2
  [ "$actual" = "$expected" ] ||
    die "authenticated GitHub user must match epic PR author $expected"
}

parse_epic_comments() {
  local comments=$1 records=$2 trusted_login=${3:-}
  node - "$comments" "$records" "$trusted_login" <<'NODE'
const fs = require("node:fs");
const [commentsPath, recordsPath, trustedLogin] = process.argv.slice(2);
const reply = JSON.parse(fs.readFileSync(commentsPath, "utf8"));
const comments = Array.isArray(reply[0]) ? reply.flat() : reply;
if (!Array.isArray(comments) || comments.length > 10_000) {
  throw new Error("GitHub epic comments have an unsupported shape");
}
if (trustedLogin && !/^[A-Za-z0-9-]{1,39}$/.test(trustedLogin)) {
  throw new Error("GitHub epic comment author is invalid");
}
const start = "<!-- ducktape-forge-epic-entry:start -->";
const end = "<!-- ducktape-forge-epic-entry:end -->";
const oid = value => typeof value === "string" && /^[0-9a-f]{40}$/.test(value);
const seen = new Map();
const lines = [];
for (const comment of comments) {
  const body = comment?.body;
  if (typeof body !== "string" || !body.includes(start)) continue;
  if (!trustedLogin || comment?.user?.login !== trustedLogin) continue;
  if (body.split(start).length !== 2 || body.split(end).length !== 2) {
    throw new Error("GitHub epic provenance comment markers are malformed or duplicated");
  }
  const a = body.indexOf(start) + start.length;
  const b = body.indexOf(end, a);
  if (b < a) throw new Error("GitHub epic provenance comment markers are out of order");
  const raw = body.slice(a, b).trim();
  const entry = JSON.parse(raw);
  if (!entry || typeof entry.repo !== "string" || !/^[a-z0-9._-]{1,64}$/.test(entry.repo) ||
      !Number.isSafeInteger(entry.pr) || entry.pr < 1 || !oid(entry.merge) || !oid(entry.source) ||
      !oid(entry.target) || !Array.isArray(entry.commits) || !entry.commits.length ||
      entry.commits.some(commit => !commit || !oid(commit.source) || !oid(commit.mirror))) {
    throw new Error("GitHub epic provenance comment contains an invalid entry");
  }
  const key = `${entry.repo}#${entry.pr}`;
  if (seen.has(key)) {
    if (seen.get(key) !== raw) throw new Error(`GitHub epic provenance conflicts for ${key}`);
    continue;
  }
  seen.set(key, raw);
  lines.push(["E", entry.repo, entry.pr, entry.merge, entry.source, entry.target].join("\t"));
  for (const commit of entry.commits) lines.push(["C", commit.source, commit.mirror].join("\t"));
}
fs.writeFileSync(recordsPath, lines.length ? lines.join("\n") + "\n" : "");
NODE
}

write_epic_comment() {
  local entry=$1 title=$2 output=$3
  node - "$entry" "$title" "$output" <<'NODE'
const fs = require("node:fs");
const [entryPath, title, outputPath] = process.argv.slice(2);
const entry = JSON.parse(fs.readFileSync(entryPath, "utf8"));
const safeTitle = title.replaceAll("&", "&amp;").replaceAll("<", "&lt;").replaceAll(">", "&gt;");
fs.writeFileSync(outputPath,
  `### Forge PR #${entry.pr} — ${safeTitle}\n\n` +
  `Verified mirror provenance for \`${entry.repo}#${entry.pr}\`.\n\n` +
  `<!-- ducktape-forge-epic-entry:start -->\n${JSON.stringify(entry, null, 2)}\n` +
  `<!-- ducktape-forge-epic-entry:end -->`);
NODE
  neutralize_github_closing_keywords "$output" "$FORGE_REPO"
}

post_epic_comment() {
  local number=$1 comment=$2 response=$3 state_dir=$4
  node - "$comment" "$state_dir/epic-comment-payload" <<'NODE'
const fs = require("node:fs");
const [commentPath, outputPath] = process.argv.slice(2);
fs.writeFileSync(outputPath, JSON.stringify({body: fs.readFileSync(commentPath, "utf8")}));
NODE
  gh api --method POST "repos/$GH_REPO/issues/$number/comments" \
    --input "$state_dir/epic-comment-payload" >"$response" 2>"$state_dir/gh.err" ||
    die "could not append epic provenance comment: $(tr '\n' ' ' <"$state_dir/gh.err")"
  node - "$response" "$comment" <<'NODE'
const fs = require("node:fs");
const [responsePath, commentPath] = process.argv.slice(2);
const response = JSON.parse(fs.readFileSync(responsePath, "utf8"));
const expected = fs.readFileSync(commentPath, "utf8");
if (response?.body !== expected) throw new Error("GitHub returned a different epic provenance comment");
NODE
}

query_epic_pr() {
  local out=$1
  gh pr list --repo "$GH_REPO" --state all --head "$MIRROR_BRANCH" --limit 100 \
    --json number,url,state,isDraft,baseRefName,headRefName,body,author >"$out"
}

parse_epic_pr() {
  local json=$1 fields=$2 body=$3
  node - "$json" "$MIRROR_BRANCH" "$fields" "$body" <<'NODE'
const fs = require("node:fs");
const [jsonPath, branch, fieldsPath, bodyPath] = process.argv.slice(2);
const prs = JSON.parse(fs.readFileSync(jsonPath, "utf8"));
if (!Array.isArray(prs) || prs.length > 1) throw new Error("epic branch has multiple GitHub PR records");
if (!prs.length) {
  fs.writeFileSync(fieldsPath, "");
  fs.writeFileSync(bodyPath, "");
  process.exit(0);
}
const pr = prs[0];
if (pr.state !== "OPEN" || pr.isDraft !== true || pr.baseRefName !== "dev" || pr.headRefName !== branch) {
  throw new Error("epic PR must be an open draft with the requested head and dev base");
}
if (typeof pr.body !== "string") throw new Error("epic PR body is unavailable");
if (!/^[A-Za-z0-9-]{1,39}$/.test(pr.author?.login || "")) {
  throw new Error("epic PR author is unavailable");
}
fs.writeFileSync(fieldsPath, `${pr.number}\n${pr.url}\n${pr.author.login}\n`);
fs.writeFileSync(bodyPath, pr.body);
NODE
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
  [ -z "${EPIC_TMP_REF:-}" ] || git update-ref -d "$EPIC_TMP_REF" >/dev/null 2>&1 || true
  if [ -n "${RUN_DIR:-}" ] && [ -d "$RUN_DIR" ]; then
    rm -f "$RUN_DIR"/*
    rmdir "$RUN_DIR" >/dev/null 2>&1 || warn "temporary state remains at $RUN_DIR"
  fi
  exit "$rc"
}

main() {
  [ "$#" -eq 2 ] || [ "$#" -eq 3 ] ||
    die "usage: ops/mirror-forge-pr.sh <forge-pr-number> <source-head-oid> [epic-branch-or-slug]"
  PR_NUMBER=$1
  SOURCE_OID=${2,,}
  EPIC_MODE=0
  if [ "$#" -eq 3 ]; then
    EPIC_MODE=1
    normalize_epic_branch "$3"
  fi
  [[ "$PR_NUMBER" =~ ^[1-9][0-9]*$ ]] || die "Forge PR number must be a positive integer"
  is_oid "$SOURCE_OID" || die "source commit must be 40 hex characters"
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
  FORGE_TMP_REF="refs/mirror-tmp/$token/forge-dev"
  ORIGIN_TMP_REF="refs/mirror-tmp/$token/origin-dev"
  EPIC_TMP_REF="refs/mirror-tmp/$token/epic"
  if [ "$EPIC_MODE" -eq 1 ]; then
    MIRROR_BRANCH=$EPIC_BRANCH
  else
    MIRROR_BRANCH="mirror/forge-pr-$PR_NUMBER-$short"
  fi
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
  log "fetching Forge dev at one unique temporary ref"
  git fetch --no-tags "$FORGE_URL" "refs/heads/dev:$FORGE_TMP_REF"
  local forge_dev
  forge_dev=$(git rev-parse "$FORGE_TMP_REF^{commit}")
  git merge-base --is-ancestor "$MERGE_OID" "$forge_dev" ||
    die "Forge merge $MERGE_OID is not in canonical Forge dev"
  local forge_target
  forge_target=$(validate_merged_selection "$MERGE_OID" "$SOURCE_OID")
  validate_commit_range "$forge_target" "$SOURCE_OID"

  log "fetching exact GitHub dev base"
  git fetch --no-tags origin "refs/heads/dev:$ORIGIN_TMP_REF"
  local origin_oid
  origin_oid=$(git rev-parse "$ORIGIN_TMP_REF^{commit}")

  local replay_base=$origin_oid epic_history_base=$origin_oid
  local epic_tip="" epic_pr_number="" epic_pr_url="" epic_pr_author=""
  local epic_recovery=0 epic_repeat=0
  local -a MIRROR_COMMITS=()
  if [ "$EPIC_MODE" -eq 1 ]; then
    gh auth status -h github.com >/dev/null 2>&1 ||
      die "gh is not authenticated for github.com (authenticate before mirroring)"
    query_epic_pr "$RUN_DIR/epic-pr.json"
    parse_epic_pr "$RUN_DIR/epic-pr.json" "$RUN_DIR/epic-pr-fields" "$RUN_DIR/epic-body"
    local -a epic_pr_fields=()
    mapfile -t epic_pr_fields <"$RUN_DIR/epic-pr-fields"
    if [ "${#epic_pr_fields[@]}" -gt 0 ]; then
      [ "${#epic_pr_fields[@]}" -eq 3 ] || die "GitHub epic PR metadata was incomplete"
      epic_pr_number=${epic_pr_fields[0]}
      epic_pr_url=${epic_pr_fields[1]}
      epic_pr_author=${epic_pr_fields[2]}
      local github_login
      github_login=$(gh api user --jq .login) || die "could not read the authenticated GitHub login"
      require_epic_author "$epic_pr_author" "$github_login"
      query_epic_comments "$epic_pr_number" "$RUN_DIR/epic-comments.json"
    else
      printf '[]\n' >"$RUN_DIR/epic-comments.json"
      printf '%s\n' \
        'Draft improvement epic for verified Forge changes.' \
        '' \
        'Each append is recorded in the machine-maintained provenance ledger below. Final clean-context review and merge remain manual.' \
        >"$RUN_DIR/epic-body"
    fi
    parse_epic_comments "$RUN_DIR/epic-comments.json" "$RUN_DIR/epic-comment-records" \
      "$epic_pr_author"

    local listed_tip=""
    listed_tip=$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH") || true
    if [ -n "$listed_tip" ]; then
      git fetch --no-tags origin "refs/heads/$MIRROR_BRANCH:$EPIC_TMP_REF"
      epic_tip=$(git rev-parse "$EPIC_TMP_REF^{commit}")
      [ "$epic_tip" = "$listed_tip" ] || die "epic branch moved while it was fetched; retry"
      [ -n "$epic_pr_number" ] || warn "epic branch exists without a PR; only exact partial-create recovery is allowed"
      epic_history_base=$(resolve_epic_history_base "$origin_oid" "$epic_tip")
      replay_base=$epic_tip
    else
      [ -z "$epic_pr_number" ] || die "epic PR exists but its branch is missing"
    fi
  fi
  assert_shared_dev_base "$forge_target" "$replay_base"
  log "shared dev history: Forge target $forge_target is represented by GitHub $replay_base"

  git worktree add --detach "$MIRROR_WORKTREE" "$replay_base" >/dev/null
  WORKTREE_ADDED=1
  assert_clean "$MIRROR_WORKTREE"

  local commit mirror_tip
  if [ "$EPIC_MODE" -eq 1 ]; then
    parse_epic_ledger "$RUN_DIR/epic-body" "$RUN_DIR/epic-records"
    cat "$RUN_DIR/epic-comment-records" >>"$RUN_DIR/epic-records"
    local -a branch_commits=() ledger_mirrors=() current_sources=() current_mirrors=()
    mapfile -t branch_commits < <(git rev-list --reverse --topo-order "$epic_history_base..$replay_base")
    local previous=$epic_history_base branch_commit parent_line
    local -a branch_parents=()
    for branch_commit in "${branch_commits[@]}"; do
      parent_line=$(git show -s --format=%P "$branch_commit")
      read -r -a branch_parents <<<"$parent_line"
      [ "${#branch_parents[@]}" -eq 1 ] && [ "${branch_parents[0]}" = "$previous" ] ||
        die "epic branch history is not one linear fast-forward chain at $branch_commit"
      previous=$branch_commit
    done
    local record_kind record_a record_b record_c record_d record_e entry_key in_current=0 current_seen=0
    local entry_open=0 entry_source_index=0 entry_label=""
    local -a entry_expected_sources=()
    local -A seen_entries=()
    local -a selected_source_commits=("${SOURCE_COMMITS[@]}")
    : >"$RUN_DIR/epic-records-resolved"
    while IFS=$'\t' read -r record_kind record_a record_b record_c record_d record_e; do
      [ -n "$record_kind" ] || continue
      if [ "$record_kind" = E ]; then
        if [ "$entry_open" -eq 1 ]; then
          [ "$entry_source_index" -eq "${#entry_expected_sources[@]}" ] ||
            die "epic provenance for $entry_label has a partial commit range"
        fi
        entry_key="$record_a#$record_b"
        [ -z "${seen_entries[$entry_key]+x}" ] || die "epic provenance duplicates $entry_key"
        seen_entries[$entry_key]=1
        validate_epic_item "$record_a" "$record_b" "$record_c" "$record_d" "$record_e" \
          "$forge_dev" "$RUN_DIR/epic-item-$record_b.json"
        entry_expected_sources=("${VALIDATED_EPIC_COMMITS[@]}")
        SOURCE_COMMITS=("${selected_source_commits[@]}")
        entry_open=1
        entry_source_index=0
        entry_label="$record_a#$record_b"
        printf 'E\t%s\t%s\t%s\t%s\t%s\n' \
          "$record_a" "$record_b" "$record_c" "$record_d" "$record_e" \
          >>"$RUN_DIR/epic-records-resolved"
        in_current=0
        if [ "$record_a" = "$FORGE_REPO" ] && [ "$record_b" = "$PR_NUMBER" ]; then
          current_seen=1
          in_current=1
          [ "$record_c" = "$MERGE_OID" ] && [ "$record_d" = "$SOURCE_OID" ] &&
            [ "$record_e" = "$forge_target" ] ||
            die "epic provenance for $FORGE_REPO#$PR_NUMBER does not match the selected Forge PR"
        fi
      elif [ "$record_kind" = C ]; then
        [ "$entry_open" -eq 1 ] || die "epic provenance commit has no owning Forge PR"
        [ "$entry_source_index" -lt "${#entry_expected_sources[@]}" ] &&
          [ "$record_a" = "${entry_expected_sources[$entry_source_index]}" ] ||
          die "epic provenance for $entry_label names the wrong source commit"
        entry_source_index=$((entry_source_index + 1))
        printf 'C\t%s\t%s\n' "$record_a" "$record_b" \
          >>"$RUN_DIR/epic-records-resolved"
        ledger_mirrors+=("$record_b")
        verify_replayed_commit "$record_a" "$record_b" "$RUN_DIR" "epic ledger"
        if [ "$in_current" -eq 1 ]; then
          current_sources+=("$record_a")
          current_mirrors+=("$record_b")
        fi
      elif [ "$record_kind" = L ]; then
        if [ "$entry_open" -eq 1 ]; then
          [ "$entry_source_index" -eq "${#entry_expected_sources[@]}" ] ||
            die "epic provenance for $entry_label has a partial commit range"
        fi
        entry_open=0
        in_current=0
        entry_key="$record_a#$record_b"
        [ -z "${seen_entries[$entry_key]+x}" ] || die "epic provenance duplicates $entry_key"
        seen_entries[$entry_key]=1
        validate_epic_item "$record_a" "$record_b" "$record_c" "$record_d" "$record_e" \
          "$forge_dev" "$RUN_DIR/epic-item-$record_b.json"
        local -a legacy_sources=("${VALIDATED_EPIC_COMMITS[@]}")
        SOURCE_COMMITS=("${selected_source_commits[@]}")
        if [ "$record_a" = "$FORGE_REPO" ] && [ "$record_b" = "$PR_NUMBER" ]; then
          current_seen=1
          [ "$record_c" = "$MERGE_OID" ] && [ "$record_d" = "$SOURCE_OID" ] &&
            [ "$record_e" = "$forge_target" ] ||
            die "legacy epic provenance for $FORGE_REPO#$PR_NUMBER does not match the selected Forge PR"
          in_current=1
        fi
        printf 'E\t%s\t%s\t%s\t%s\t%s\n' \
          "$record_a" "$record_b" "$record_c" "$record_d" "$record_e" \
          >>"$RUN_DIR/epic-records-resolved"
        local legacy_source legacy_mirror branch_index
        for legacy_source in "${legacy_sources[@]}"; do
          branch_index=${#ledger_mirrors[@]}
          [ "$branch_index" -lt "${#branch_commits[@]}" ] ||
            die "legacy epic provenance names history missing from the epic branch"
          legacy_mirror=${branch_commits[$branch_index]}
          verify_replayed_commit "$legacy_source" "$legacy_mirror" "$RUN_DIR" \
            "legacy epic provenance"
          ledger_mirrors+=("$legacy_mirror")
          printf 'C\t%s\t%s\n' "$legacy_source" "$legacy_mirror" \
            >>"$RUN_DIR/epic-records-resolved"
          if [ "$in_current" -eq 1 ]; then
            current_sources+=("$legacy_source")
            current_mirrors+=("$legacy_mirror")
          fi
        done
      else
        die "epic provenance parser returned an invalid record"
      fi
    done <"$RUN_DIR/epic-records"
    if [ "$entry_open" -eq 1 ]; then
      [ "$entry_source_index" -eq "${#entry_expected_sources[@]}" ] ||
        die "epic provenance for $entry_label has a partial commit range"
    fi
    [ "${#ledger_mirrors[@]}" -le "${#branch_commits[@]}" ] ||
      die "epic provenance ledger names commits not present on the branch"
    local i
    for ((i = 0; i < ${#ledger_mirrors[@]}; i += 1)); do
      [ "${ledger_mirrors[$i]}" = "${branch_commits[$i]}" ] ||
        die "epic provenance ledger does not match branch history at ${branch_commits[$i]}"
    done
    if [ "$current_seen" -eq 1 ]; then
      [ "${#ledger_mirrors[@]}" -eq "${#branch_commits[@]}" ] ||
        die "epic branch has history missing from its provenance ledger"
      [ "${#current_sources[@]}" -eq "${#SOURCE_COMMITS[@]}" ] ||
        die "epic provenance for $FORGE_REPO#$PR_NUMBER has a partial commit range"
      for ((i = 0; i < ${#SOURCE_COMMITS[@]}; i += 1)); do
        [ "${current_sources[$i]}" = "${SOURCE_COMMITS[$i]}" ] ||
          die "epic provenance for $FORGE_REPO#$PR_NUMBER names the wrong source commit"
      done
      MIRROR_COMMITS=("${current_mirrors[@]}")
      mirror_tip=$replay_base
      epic_repeat=1
    else
      local uncovered=$((${#branch_commits[@]} - ${#ledger_mirrors[@]}))
      if [ "$uncovered" -gt 0 ]; then
        [ "$uncovered" -eq "${#SOURCE_COMMITS[@]}" ] ||
          die "epic branch has partial or unrelated history missing from its provenance ledger"
        for ((i = 0; i < uncovered; i += 1)); do
          local branch_index=$((${#ledger_mirrors[@]} + i))
          verify_replayed_commit "${SOURCE_COMMITS[$i]}" "${branch_commits[$branch_index]}" \
            "$RUN_DIR" "partial-success recovery"
          MIRROR_COMMITS+=("${branch_commits[$branch_index]}")
        done
        [ -n "$epic_pr_number" ] ||
          [ "${#ledger_mirrors[@]}" -eq 0 ] || die "epic branch without a PR contains prior ledger history"
        mirror_tip=$replay_base
        epic_recovery=1
      else
        for commit in "${SOURCE_COMMITS[@]}"; do
          log "appending $commit to epic with its original message and identities"
          replay_commit "$MIRROR_WORKTREE" "$commit" "$RUN_DIR"
          MIRROR_COMMITS+=("$MIRRORED_OID")
        done
        mirror_tip=$MIRRORED_OID
      fi
    fi
  else
    for commit in "${SOURCE_COMMITS[@]}"; do
      log "replaying $commit with its original message and identities"
      replay_commit "$MIRROR_WORKTREE" "$commit" "$RUN_DIR"
    done
    mirror_tip=$MIRRORED_OID
  fi
  git -C "$MIRROR_WORKTREE" diff --quiet "$origin_oid..$mirror_tip" &&
    die "Forge PR has no net changes on GitHub dev"
  git -C "$MIRROR_WORKTREE" diff --check "$origin_oid..$mirror_tip"

  if [ "$EPIC_MODE" -eq 0 ]; then
    gh auth status -h github.com >/dev/null 2>&1 ||
      die "gh is not authenticated for github.com (authenticate before mirroring)"
  fi

  local remote_dev
  remote_dev=$(remote_ref_oid origin refs/heads/dev) || die "origin/dev is missing"
  [ "$remote_dev" = "$origin_oid" ] || die "origin/dev moved during replay; retry from the new base"
  if [ "$EPIC_MODE" -eq 0 ]; then
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
  elif [ "$epic_repeat" -eq 0 ] && [ "$epic_recovery" -eq 0 ]; then
    local before_push=""
    before_push=$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH") || true
    [ "$before_push" = "$epic_tip" ] || die "epic branch moved during replay; retry"
    log "fast-forwarding $MIRROR_BRANCH to $mirror_tip"
    # This deliberately is not a force push. A concurrent update or a rewrite
    # makes the ref update fail, and the exact post-push check catches an
    # identical/up-to-date race as well.
    git -C "$MIRROR_WORKTREE" push origin "HEAD:refs/heads/$MIRROR_BRANCH"
    PUSHED=1
  fi
  local pushed_oid
  pushed_oid=$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH") || die "pushed branch is missing"
  [ "$pushed_oid" = "$mirror_tip" ] || die "pushed branch does not match the verified mirror commit"
  remote_dev=$(remote_ref_oid origin refs/heads/dev) || die "origin/dev disappeared after push"
  [ "$remote_dev" = "$origin_oid" ] || die "origin/dev moved while pushing; retry from the new base"

  query_item "$RUN_DIR/item-recheck.json"
  cmp -s "$RUN_DIR/item.json" "$RUN_DIR/item-recheck.json" ||
    die "Forge PR metadata changed during mirroring; retry"

  if [ "$EPIC_MODE" -eq 1 ]; then
    [ "$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH")" = "$mirror_tip" ] ||
      die "epic branch moved before its provenance ledger update; retry"
    if [ "$epic_repeat" -eq 1 ]; then
      [ -n "$epic_pr_number" ] || die "repeated epic provenance has no open draft PR"
      query_epic_pr "$RUN_DIR/epic-pr-final.json"
      cmp -s "$RUN_DIR/epic-pr.json" "$RUN_DIR/epic-pr-final.json" ||
        die "epic PR changed during idempotence verification; retry"
      query_epic_comments "$epic_pr_number" "$RUN_DIR/epic-comments-final.json"
      cmp -s "$RUN_DIR/epic-comments.json" "$RUN_DIR/epic-comments-final.json" ||
        die "epic comments changed during idempotence verification; retry"
      [ "$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH")" = "$mirror_tip" ] ||
        die "epic branch moved during idempotence verification; retry"
      log "already appended and verified: $epic_pr_url"
      log "epic finalization remains a separate manual clean-context review and merge-commit action"
      return
    fi

    : >"$RUN_DIR/epic-mappings"
    local i
    for ((i = 0; i < ${#SOURCE_COMMITS[@]}; i += 1)); do
      printf '%s\t%s\n' "${SOURCE_COMMITS[$i]}" "${MIRROR_COMMITS[$i]}" >>"$RUN_DIR/epic-mappings"
    done
    node - "$RUN_DIR/epic-mappings" "$RUN_DIR/epic-entry" "$FORGE_REPO" "$PR_NUMBER" \
      "$MERGE_OID" "$SOURCE_OID" "$forge_target" <<'NODE'
const fs = require("node:fs");
const [mappingsPath, outputPath, repo, pr, merge, source, target] = process.argv.slice(2);
const commits = fs.readFileSync(mappingsPath, "utf8").trim().split("\n").filter(Boolean).map(line => {
  const [source, mirror] = line.split("\t");
  return {source, mirror};
});
fs.writeFileSync(outputPath, JSON.stringify({repo, pr: Number(pr), merge, source, target, commits}));
NODE
    if [ -n "$epic_pr_number" ]; then
      write_epic_comment "$RUN_DIR/epic-entry" "$PR_TITLE" "$RUN_DIR/epic-comment"
      query_epic_comments "$epic_pr_number" "$RUN_DIR/epic-comments-before-post.json"
      cmp -s "$RUN_DIR/epic-comments.json" "$RUN_DIR/epic-comments-before-post.json" ||
        die "epic comments changed before the provenance append; retry"
      post_epic_comment "$epic_pr_number" "$RUN_DIR/epic-comment" \
        "$RUN_DIR/epic-comment-response.json" "$RUN_DIR"
      PR_CREATED=1
      node - "$RUN_DIR/epic-comment-response.json" "$RUN_DIR/epic-comment-response-pages.json" <<'NODE'
const fs = require("node:fs");
const [inputPath, outputPath] = process.argv.slice(2);
fs.writeFileSync(outputPath, JSON.stringify([[JSON.parse(fs.readFileSync(inputPath, "utf8"))]]));
NODE
      parse_epic_comments "$RUN_DIR/epic-comment-response-pages.json" \
        "$RUN_DIR/epic-current-comment-records" "$epic_pr_author"
      cat "$RUN_DIR/epic-comment-records" "$RUN_DIR/epic-current-comment-records" \
        >"$RUN_DIR/epic-comment-records-expected"
      query_epic_comments "$epic_pr_number" "$RUN_DIR/epic-comments-final.json"
      parse_epic_comments "$RUN_DIR/epic-comments-final.json" \
        "$RUN_DIR/epic-comment-records-final" "$epic_pr_author"
      cmp -s "$RUN_DIR/epic-comment-records-expected" "$RUN_DIR/epic-comment-records-final" ||
        die "epic provenance comments changed during the append; inspect before retrying"
    else
      append_epic_ledger "$RUN_DIR/epic-body" "$RUN_DIR/epic-entry" \
        "$RUN_DIR/epic-body-updated" "$RUN_DIR/epic-records-resolved"
      local epic_title="Improvement epic: $3"
      gh pr create --draft --repo "$GH_REPO" --base dev --head "$MIRROR_BRANCH" \
        --title "$epic_title" --body-file "$RUN_DIR/epic-body-updated" \
        >"$RUN_DIR/gh.out" 2>"$RUN_DIR/gh.err" ||
        die "gh pr create failed: $(tr '\n' ' ' <"$RUN_DIR/gh.err")"
      PR_CREATED=1
    fi
    query_epic_pr "$RUN_DIR/epic-pr-final.json"
    parse_epic_pr "$RUN_DIR/epic-pr-final.json" "$RUN_DIR/epic-pr-final-fields" "$RUN_DIR/epic-body-final"
    if [ -n "$epic_pr_number" ]; then
      cmp -s "$RUN_DIR/epic-body" "$RUN_DIR/epic-body-final" ||
        die "epic PR body changed while its provenance comment was appended; inspect before retrying"
    else
      cmp -s "$RUN_DIR/epic-body-updated" "$RUN_DIR/epic-body-final" ||
        die "epic PR body does not contain the exact verified provenance ledger"
    fi
    [ "$(remote_ref_oid origin "refs/heads/$MIRROR_BRANCH")" = "$mirror_tip" ] ||
      die "epic branch moved while its provenance ledger was updated; retry"
    local -a final_fields=()
    mapfile -t final_fields <"$RUN_DIR/epic-pr-final-fields"
    [ "${#final_fields[@]}" -eq 3 ] || die "updated epic PR metadata is incomplete"
    [ -z "$epic_pr_author" ] || [ "${final_fields[2]}" = "$epic_pr_author" ] ||
      die "epic PR author changed during provenance verification"
    log "draft epic: ${final_fields[1]}"
    if [ "$epic_recovery" -eq 1 ]; then
      log "recovered an already-pushed exact replay by repairing only its missing ledger entry"
    fi
    log "epic finalization remains a separate manual clean-context review and merge-commit action"
    return
  fi

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
