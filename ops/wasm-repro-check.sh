#!/usr/bin/env bash
# make wasm-repro-check — prove a guest component's bytes do not depend on
# WHERE the checkout lives.
#
# Every committed component.wasm used to embed its builder's absolute paths —
# `/home/<user>/.cargo/registry/...` in panic locations, and the checkout path
# inside every symbol hash — so no two checkouts produced the same bytes: a
# `make wasm-modules` anywhere rewrote all ~40 artifacts, and every module PR
# needed artifact-revert gymnastics. guest-builder now remaps the tool prefixes
# away AND compiles a snapshot of the checkout from inside its scratch
# workspace (bin/guest-builder/src/main.rs: `remap_flags`, `snapshot`).
#
# It builds ONE module (the smallest — every module shares the same prefixes)
# twice: once from this checkout, once from a copy of the tree at a different
# absolute path, and asserts BOTH that the two artifacts are byte-identical and
# that neither carries a host path. Both assertions earn their keep: on one box
# the two builds share $HOME, so a dropped remap flag leaves them identical to
# each other and only the host-path scan catches it.
#
# Needs the wasm32-unknown-unknown target and wasm-tools, so it is NOT part of
# `make test`; `make wasm-modules-check` carries the cheap host-path half.
set -euo pipefail

MODULE=${MODULE:-crates/examples/directory}
repo=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
# under target/, not /tmp: /tmp is memory-backed on some dev boxes and this
# holds a source copy plus its own wasm32 target dir. Wiped on ENTRY, not on
# exit, so a failure leaves both artifacts to compare.
work="$repo/target/wasm-repro"
rm -rf "$work"

# the copy is a DIFFERENT absolute path, which is the whole point; target/ and
# .git are build output and history, neither of which a build reads.
copy="$work/checkout-at-another-path"
mkdir -p "$copy"
tar -cf - -C "$repo" --exclude=./target --exclude=./.git --exclude=./.worktree . |
  tar -xf - -C "$copy"

cargo build -q -p guest-builder
builder="${CARGO_TARGET_DIR:-$repo/target}/debug/guest-builder"

"$builder" --platform-root "$repo" "$repo/$MODULE" --out "$work/here.wasm"
"$builder" --platform-root "$copy" "$copy/$MODULE" --out "$work/there.wasm"

if ! cmp "$work/here.wasm" "$work/there.wasm"; then
  echo "wasm-repro-check: $MODULE built at two paths differs — a builder-local" >&2
  echo "path reached the artifact. Compare the embedded strings:" >&2
  echo "  strings $work/here.wasm | grep '^/'" >&2
  exit 1
fi

# `grep | head` exits 0 either way, so the emptiness of the capture is the
# verdict — never the pipeline's status.
leak=$(grep -aoE '/(home|Users)/[^ ]*' "$work/here.wasm" | tr -d '\0' | sort -u | head -5 || true)
if [ -n "$leak" ]; then
  echo "wasm-repro-check: host paths embedded in $MODULE:" >&2
  echo "$leak" >&2
  exit 1
fi

echo "wasm-repro-check: $MODULE is byte-identical from two checkout paths, no host path embedded"
