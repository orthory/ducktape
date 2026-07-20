#!/usr/bin/env bash
# Reap finished worktrees and the QA fleet instances they leave behind.
#
# THE LEAK THIS EXISTS TO CLOSE. A Fleet instance's teardown hook
# (`cleanupInstance` in .tauri-agent/fleet.json) is a path INSIDE the worktree
# — `qa/fleet/cleanup-instance.ts`. Its workspace, pidfile and detached
# `ducktape-node` live OUTSIDE it, under FLEET_HOME. So deleting a worktree
# deletes the only thing that could have stopped its node, and the node then
# runs forever: we found one still up 40 hours after its worktree was gone, and
# 9.2 GB of instance homes belonging to worktrees that no longer existed.
#
# `fleet down <id>` is the correct teardown and it works. The failure is one of
# ORDER: remove the worktree first and there is nothing left to run. So the
# sequence is always stop-then-remove, and this script is that sequence.
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
    -h|--help) sed -n '2,26p' "$0" | sed 's/^# \?//'; exit 0 ;;
    *) echo "unknown flag: $arg" >&2; exit 2 ;;
  esac
done

PRIMARY="$(pwd -P)"
FLEET_ROOT="${FLEET_ROOT:-$HOME/.local/opt/remote-tauri/fleet}"
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

# ── kill one Fleet instance's node, identity-verified ────────────────────────
# Mirrors qa/fleet/cleanup-instance.ts: confirm the pid really IS the node for
# THIS workspace (exe + --config) before signalling, so a recycled pid is never
# the victim. Signals the process GROUP — the node detaches children.
kill_instance_node() {
  local workspace="$1" pid exe cfg pgid
  pid=$(cat "$workspace/node.pid" 2>/dev/null) || return 0
  [[ "$pid" =~ ^[0-9]+$ ]] && [ "$pid" -gt 1 ] || { run rm -f "$workspace/node.pid"; return 0; }
  [ -d "/proc/$pid" ] || { say "stale pidfile (pid $pid gone)"; run rm -f "$workspace/node.pid"; return 0; }

  exe=$(readlink -f "/proc/$pid/exe" 2>/dev/null)
  cfg=$(tr '\0' '\n' < "/proc/$pid/cmdline" 2>/dev/null | grep -A1 -x -- '--config' | tail -1)
  # `*ducktape-node` matches pre-rename zombies; `*ducktape` the unified CLI.
  case "$exe" in *ducktape-node|*ducktape) ;; *)
    say "REFUSING to kill pid $pid — not a ducktape node ($exe)"; return 0 ;;
  esac
  if [ "$(readlink -f "$cfg" 2>/dev/null)" != "$(readlink -f "$workspace/node.toml" 2>/dev/null)" ]; then
    say "REFUSING to kill pid $pid — its --config is not this workspace's"; return 0
  fi

  pgid=$(awk '{print $5}' "/proc/$pid/stat" 2>/dev/null)
  say "stopping node pid $pid (pgid $pgid)"
  if [ "$YES" = 1 ]; then
    kill -TERM "-$pgid" 2>/dev/null
    for _ in $(seq 20); do kill -0 "-$pgid" 2>/dev/null || break; sleep 0.25; done
    kill -0 "-$pgid" 2>/dev/null && kill -KILL "-$pgid" 2>/dev/null
    rm -f "$workspace/node.pid"
  fi
}

# ── 1. orphaned fleet instance homes ─────────────────────────────────────────
# An instance whose worktree is gone can never be torn down by Fleet, because
# the hook went with it. This is the only thing that can still stop it.
echo "Orphaned Fleet instances under $FLEET_ROOT:"
found=0
for home in "$FLEET_ROOT"/*/; do
  [ -d "$home" ] || continue
  id=$(basename "$home")
  # An instance is orphaned when no worktree of this repo still claims its id.
  if git worktree list --porcelain | grep -q "^worktree .*/$id\$"; then continue; fi
  found=1
  echo "- $id ($(du -sh "$home" 2>/dev/null | cut -f1))"
  for ws in "$home"/home/.ducktape/workspaces/*/; do
    [ -d "$ws" ] && kill_instance_node "${ws%/}"
  done
  say "rm -rf $home"
  run rm -rf "$home"
done
[ "$found" = 0 ] && echo "  none"

# ── 2. finished worktrees ────────────────────────────────────────────────────
echo
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
