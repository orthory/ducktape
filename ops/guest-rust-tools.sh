#!/usr/bin/env bash
# ROOTFS_SETUP hook: install a guest-local Rust compiler, wasm-tools and the
# ordinary native linker/build utilities. No host home or build cache crosses.
# Usage: ROOTFS_SETUP=ops/guest-rust-tools.sh ops/build-guest-rootfs.sh <rust> <wasm-tools>
set -euo pipefail
RUST_CHANNEL="${1:?supply the Rust channel from rust-toolchain.toml}"
WASM_TOOLS_VERSION="${2:?supply the wasm-tools version}"

mkdir -p /var/cache/apt/archives/partial /var/lib/apt/lists/partial /var/log/apt
apt-get -o APT::Sandbox::User=root update
apt-get -o APT::Sandbox::User=root install -y --no-install-recommends build-essential git ca-certificates curl pkg-config
apt-get clean
rm -rf /var/lib/apt/lists/*

export RUSTUP_HOME=/tmp/guest-rustup
export CARGO_HOME=/tmp/guest-cargo
export CARGO_TARGET_DIR=/tmp/guest-cargo-target
export CARGO_BUILD_JOBS=4
ARCH="$(uname -m)"
curl --fail --location --retry 3 "https://static.rust-lang.org/rustup/dist/$ARCH-unknown-linux-gnu/rustup-init" -o /tmp/rustup-init
chmod 0755 /tmp/rustup-init
/tmp/rustup-init -y --no-modify-path --profile minimal --default-toolchain "$RUST_CHANNEL"
export PATH="$CARGO_HOME/bin:$PATH"
rustup target add wasm32-unknown-unknown
cargo install wasm-tools --version "$WASM_TOOLS_VERSION" --locked --root /usr/local

# Direct toolchain binaries work in a fresh guest HOME. Rustup's selection and
# download state is setup scratch, so no runtime needs a network to select Rust.
mkdir -p /opt/rust
cp -a "$RUSTUP_HOME/toolchains/$RUST_CHANNEL-$ARCH-unknown-linux-gnu/." /opt/rust/
for command in cargo rustc rustdoc; do
  ln -sfn "/opt/rust/bin/$command" "/usr/local/bin/$command"
done
/usr/local/bin/rustc --version
/usr/local/bin/cargo --version
/usr/local/bin/wasm-tools --version
