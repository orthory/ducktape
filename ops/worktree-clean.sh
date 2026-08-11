#!/usr/bin/env bash
# Reap worktrees whose work is fully merged into origin/dev.
#
# A worktree's life ends when its PR merges; what it leaves behind is ~20 GB of
# Cargo target and nothing else. Twelve of them once ate 250 GB.
#
# Dry-run by default. It removes worktrees; that is not something to do on a
# typo. Pass --yes to act.
#
#   ops/worktree-clean.sh              # show what would be reaped
#   ops/worktree-clean.sh --yes        # do it
#   ops/worktree-clean.sh --yes --force  # also take worktrees that have live processes
#
# Refuses, always, to touch a worktree that is dirty or carries a commit not in
# origin/dev. Unmerged work is never this script's to throw away.
set -uo pipefail

cd "$(git rev-parse --show-toplevel)" || exit 1

YES=0
FORCE=0
for arg in "$@"; do
  case "$arg" in
    --yes) YES=1 ;;
    --force) FORCE=1 ;;
    -h|--help) sed -n '2,15p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

PRIMARY="$(pwd -P)"
say() { [ "$YES" = 1 ] && echo "  $*" || echo "  [dry-run] $*"; }
run() { [ "$YES" = 1 ] && "$@"; }

git fetch -q origin dev 2>/dev/null || true

# ── does anything live in this directory? ────────────────────────────────────
# By cwd, not `pkill -f`: a pattern match would happily kill a process that
# merely mentions the path (this script, an editor, a grep). The QA skill says
# the same thing in more words — "Never find or stop desktop processes with
# pkill -f".
pids_under() {
  local dir="$1" p cwd
  for p in $(ls /proc 2>/dev/null | grep -E '^[0-9]+$'); do
    cwd=$(readlink "/proc/$p/cwd" 2>/dev/null) || continue
    case "$cwd" in "$dir"|"$dir"/*) echo "$p" ;; esac
  done
}

echo "Worktrees whose work is fully in origin/dev:"
found=0
# Process substitution, not a pipe: a piped `while` runs in a subshell, so
# `found` would never survive the loop and the summary would always say "none".
while read -r wt; do
  [ "$wt" = "$PRIMARY" ] && continue
  [ -d "$wt" ] || continue
  branch=$(git -C "$wt" branch --show-current 2>/dev/null)
  [ -n "$branch" ] || continue

  dirty=$(git -C "$wt" status --porcelain 2>/dev/null | wc -l)
  ahead=$(git -C "$wt" rev-list --count origin/dev..HEAD 2>/dev/null || echo 1)
  live=$(pids_under "$wt" | wc -w)

  # The two refusals that make this safe to run without reading the code.
  if [ "$ahead" != "0" ]; then
    echo "- SKIP $branch — $ahead commit(s) not in dev"; continue
  fi
  if [ "$dirty" != "0" ]; then
    echo "- SKIP $(basename "$wt") — $dirty uncommitted change(s)"; continue
  fi
  if [ "$live" != "0" ] && [ "$FORCE" != 1 ]; then
    echo "- SKIP $(basename "$wt") — $live live process(es); stop them or pass --force"; continue
  fi

  found=1
  echo "- $(basename "$wt")  [$branch]  $(du -sh "$wt" 2>/dev/null | cut -f1)"
  if [ "$live" != "0" ]; then
    say "killing $live process(es) under it (--force)"
    [ "$YES" = 1 ] && { kill -TERM $(pids_under "$wt") 2>/dev/null; sleep 2; kill -KILL $(pids_under "$wt") 2>/dev/null; }
  fi
  say "git worktree remove --force $wt"
  run git worktree remove --force "$wt"
  say "git branch -d $branch"
  run git branch -d "$branch"
done < <(git worktree list --porcelain | awk '/^worktree /{print $2}')
[ "$found" = 0 ] && echo "  none"

run git worktree prune
echo
[ "$YES" = 1 ] || echo "Nothing was changed. Re-run with --yes to apply."
