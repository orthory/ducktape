#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER="$ROOT/ops/build-with.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

fail() {
  echo "build-with test failed: $*" >&2
  exit 1
}

make_case() {
  case_dir="$1"
  os="$2"
  host="$3"
  mkdir -p "$case_dir/bin"

  cat >"$case_dir/bin/uname" <<EOF
#!/usr/bin/env bash
printf '%s\n' '$os'
EOF
  cat >"$case_dir/bin/rustc" <<EOF
#!/usr/bin/env bash
cat <<'VERSION'
rustc 1.96.0
host: $host
VERSION
EOF
  cat >"$case_dir/bin/cargo" <<'EOF'
#!/usr/bin/env bash
{
  printf 'args=%s\n' "$*"
  printf 'RUSTC_WRAPPER=%s\n' "${RUSTC_WRAPPER:-}"
  printf 'SCCACHE_IGNORE_SERVER_IO_ERROR=%s\n' "${SCCACHE_IGNORE_SERVER_IO_ERROR:-}"
  printf 'SCCACHE_BASEDIRS=%s\n' "${SCCACHE_BASEDIRS:-}"
  env | LC_ALL=C sort | sed -n '/^CARGO_TARGET_.*_LINKER=/p; /^CARGO_TARGET_.*_RUSTFLAGS=/p'
} >"$BUILD_WITH_TEST_LOG"
EOF
  for tool in sccache mold clang; do
    cat >"$case_dir/bin/$tool" <<EOF
#!/usr/bin/env bash
exit 0
EOF
  done
  chmod +x "$case_dir/bin/"*
}

linux="$TMP/linux"
make_case "$linux" Linux x86_64-unknown-linux-gnu
PATH="$linux/bin:/usr/bin:/bin" BUILD_WITH_TEST_LOG="$linux/log" \
  "$HELPER" cargo check -p files >/dev/null 2>&1
grep -Fq "args=check -p files" "$linux/log" || fail "cargo arguments were not preserved"
grep -Fq "RUSTC_WRAPPER=$linux/bin/sccache" "$linux/log" || fail "sccache was not enabled"
grep -Fq "SCCACHE_IGNORE_SERVER_IO_ERROR=1" "$linux/log" || fail "sccache fallback was not enabled"
grep -Fq "SCCACHE_BASEDIRS=$ROOT" "$linux/log" || fail "worktree cache roots were not normalized"
grep -Fq "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=$linux/bin/clang" "$linux/log" || fail "clang linker was not selected"
grep -Fq "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=-C link-arg=-fuse-ld=mold" "$linux/log" || fail "mold rustflags were not selected"

darwin="$TMP/darwin"
make_case "$darwin" Darwin aarch64-apple-darwin
PATH="$darwin/bin:/usr/bin:/bin" BUILD_WITH_TEST_LOG="$darwin/log" \
  "$HELPER" cargo check >/dev/null 2>&1
grep -Fq "RUSTC_WRAPPER=$darwin/bin/sccache" "$darwin/log" || fail "sccache should work on macOS"
if grep -Eq '^CARGO_TARGET_.*_(LINKER|RUSTFLAGS)=' "$darwin/log"; then
  fail "mold configuration leaked onto macOS"
fi

override="$TMP/override"
make_case "$override" Linux x86_64-unknown-linux-gnu
PATH="$override/bin:/usr/bin:/bin" BUILD_WITH_TEST_LOG="$override/log" \
  RUSTC_WRAPPER=/custom/wrapper RUSTFLAGS='-C target-cpu=native' \
  "$HELPER" cargo test >/dev/null 2>&1
grep -Fq "RUSTC_WRAPPER=/custom/wrapper" "$override/log" || fail "existing rustc wrapper was overwritten"
if grep -Eq '^CARGO_TARGET_.*_(LINKER|RUSTFLAGS)=' "$override/log"; then
  fail "mold configuration should not override RUSTFLAGS"
fi

versioned="$TMP/versioned-linker"
make_case "$versioned" Linux x86_64-unknown-linux-gnu
PATH="$versioned/bin:/usr/bin:/bin" BUILD_WITH_TEST_LOG="$versioned/log" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/custom/clang-18 \
  "$HELPER" cargo check >/dev/null 2>&1
grep -Fq "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/custom/clang-18" "$versioned/log" || fail "versioned clang selection was overwritten"
grep -Fq "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS=-C link-arg=-fuse-ld=mold" "$versioned/log" || fail "mold was not added to a versioned clang selection"

disabled="$TMP/disabled"
make_case "$disabled" Linux x86_64-unknown-linux-gnu
PATH="$disabled/bin:/usr/bin:/bin" BUILD_WITH_TEST_LOG="$disabled/log" \
  DUCKTAPE_DISABLE_SCCACHE=1 DUCKTAPE_DISABLE_MOLD=1 \
  "$HELPER" cargo check >/dev/null 2>&1
grep -Fq "RUSTC_WRAPPER=" "$disabled/log" || fail "disabled sccache still set a wrapper"
if grep -Eq '^CARGO_TARGET_.*_(LINKER|RUSTFLAGS)=' "$disabled/log"; then
  fail "disabled mold still configured the target"
fi

status="$(PATH="$linux/bin:/usr/bin:/bin" "$HELPER" --status)"
case "$status" in
  *"sccache: enabled"*"mold: enabled"*) ;;
  *) fail "status did not report enabled Linux helpers" ;;
esac

echo "build-with tests passed"
