# ducktape build + install entry points.
#
# `make install` builds the networked node and the desktop app, installs
# ducktape-node into ~/.cargo/bin, and installs the app — on macOS
# Ducktape.app into /Applications, on Linux the plain `ducktape` binary
# into ~/.cargo/bin (next to ducktape-node, which the app resolves as a
# sibling of its own executable). individual targets below for the pieces.

CARGO ?= cargo
BUN ?= bun
APP_DEST ?= /Applications
BIN_DEST ?= $(HOME)/.cargo/bin

# The desktop shell runs on tauri-runtime-cef (unreleased tauri feat/cef
# branch): EVERY cargo step needs the patched checkout wired in via
# [patch.crates-io] first. `cef-env` provisions it (idempotent) — see
# ops/cef-probe/setup.sh. CEF_PATH is where the CEF binary distribution
# lives (cef-dll-sys downloads into it on first build; the tauri CLI hands
# it to the macOS bundler for the framework/helper copy).
CEF_CLONE ?= $(HOME)/.cache/ducktape-cef-probe/tauri-cef
export CEF_PATH ?= $(HOME)/.local/share/cef

UNAME_S := $(shell uname -s)

.PHONY: all dev dogfood-forge node coordinator coordinator-smoke web app sidecar install install-node install-coordinator install-app stream-types test clean wasm-modules wasm-modules-check

all: node web

## provision the patched feat/cef tauri checkout this workspace builds
## against (clone + default-runtime flip + version bump + [patch] append).
## idempotent; every cargo-touching target depends on it.
cef-env:
	@bash ops/cef-probe/setup.sh "$(CEF_CLONE)"

## dev loop: the desktop app + a HOT-RELOADING node. runs `tauri dev` (frontend
## hot-reload) and watches the Rust tree — on any node/kernel change it rebuilds
## ducktape-node and restarts the running node in place, which the app re-adopts.
## see ops/dev.sh. (stop any already-running `tauri dev` first — it owns :1430.)
dev: cef-env
	@bash ops/dev.sh

## dogfood: host ducktape's own source in the local dev node's forge module.
## registers a static `ducktape-dev` git remote at the node's forge endpoint and
## pushes `main` (needs a running dev node — `make dev`). see ops/dogfood-forge.sh.
dogfood-forge:
	@bash ops/dogfood-forge.sh

## release build of the networked node (serves the app surface)
node: cef-env
	$(CARGO) build --release -p node-bin

## release build of the untrusted UDP coordinator
coordinator: cef-env
	$(CARGO) build --release -p coordinator-bin

## coordinator-only verification gate: CLI/policy tests + live UDP smoke
coordinator-smoke: cef-env
	$(CARGO) test -p coordinator-bin

## stage the daemon as the desktop app's sidecar (app/src-tauri/binaries)
sidecar: cef-env app/node_modules
	cd app && $(BUN) run sidecar

## static web bundle -> app/dist
web: app/node_modules
	cd app && $(BUN) run build

## desktop build — stages the sidecar itself via beforeBuildCommand. on macOS
## a bundle (.app + .dmg under target/release/bundle); on Linux the plain
## binary at target/release/ducktape-desktop (--no-bundle: install-app wants
## only the binary, and no deb/rpm/appimage packagers are needed). the dmg
## post-fix hides .VolumeIcon.icns, which macOS 26 Finder would otherwise
## show overlapping the app icon — see ops/fix-dmg.sh.
## on macOS the bundle MUST be built with the feat/cef tauri CLI: it copies
## "Chromium Embedded Framework.framework" and the CEF helper apps into the
## .app — the released npm @tauri-apps/cli knows nothing about CEF and
## produces a bundle that panics in cef::library_loader at launch.
ifeq ($(UNAME_S),Darwin)
app: cef-env app/node_modules
	cd app && $(CARGO) run --manifest-path "$(CEF_CLONE)/crates/tauri-cli/Cargo.toml" --bin cargo-tauri -- build
	bash ops/fix-dmg.sh
else
app: cef-env app/node_modules
	cd app && $(BUN) run tauri build --no-bundle
endif

# re-run bun install whenever the manifest or lockfile changes, not just when
# node_modules is absent; the touch keeps the dir newer than its prerequisites
# (bun does not reliably update the dir mtime when nothing needs fetching).
app/node_modules: app/package.json app/bun.lock
	cd app && $(BUN) install
	touch app/node_modules

install: install-node install-app

## ducktape-node -> ~/.cargo/bin
install-node: cef-env
	$(CARGO) install --path bin/node --locked

## coordinator -> ~/.cargo/bin/ducktape-coordinator
install-coordinator: cef-env
	$(CARGO) build --release -p coordinator-bin
	mkdir -p "$(BIN_DEST)"
	install -m 755 target/release/coordinator "$(BIN_DEST)/ducktape-coordinator"

## macOS: Ducktape.app -> $(APP_DEST); Linux: ducktape -> $(BIN_DEST),
## alongside install-node's ducktape-node so the app's sidecar resolution
## (a `ducktape-node` sibling of its own executable) finds it.
ifeq ($(UNAME_S),Darwin)
install-app: app
	mkdir -p "$(APP_DEST)"
	rm -rf "$(APP_DEST)/Ducktape.app"
	cp -R target/release/bundle/macos/Ducktape.app "$(APP_DEST)/"
	@echo "installed $(APP_DEST)/Ducktape.app"
else
install-app: app
	mkdir -p "$(BIN_DEST)"
	install -m 755 target/release/ducktape-desktop "$(BIN_DEST)/ducktape"
	@echo "installed $(BIN_DEST)/ducktape"
	bash ops/install-desktop-entry.sh "$(BIN_DEST)/ducktape"
endif

## regenerate app/src/domain/stream.gen.ts from the stream contract
stream-types: cef-env
	$(CARGO) test -p noded export_ts_bindings

## the full LOCAL verification gate (no hosted CI by design — run this before
## every push): the rust workspace including the process-level e2e suites
## (bin/node spawns a real 4-node cluster over localhost TCP, bin/noded drives
## a real spawned daemon over http/ws), then the app suites with the daemon
## binary staged so the live-daemon wire-parity e2e RUNS instead of skipping,
## and the sim node staged so the provider scenario suite runs too.
test: cef-env app/node_modules wasm-modules-check
	$(CARGO) test --workspace
	$(MAKE) stream-types
	git diff --exit-code -- app/src/domain/stream.gen.ts
	$(CARGO) build -p noded -p simnode
	cd app && $(BUN) run typecheck
	cd app && DUCKTAPE_NODED_BIN=$(abspath target/debug/ducktape-noded) DUCKTAPE_SIMNODE_BIN=$(abspath target/debug/ducktape-simnode) $(BUN) run test

## rebuild every wasm guest module into its componentized artifact and refresh
## EVERY committed copy in one sweep (the canonical node-embedded artifact +
## the kernel test fixtures), so the copies can never drift apart. requires
## the wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)
## and wasm-tools (cargo install wasm-tools). component bytes are toolchain-
## dependent: a rebuild on a different rustc may legitimately differ from the
## committed bytes — commit the refreshed set TOGETHER; `wasm-modules-check`
## guards mutual consistency, not reproducibility. standalone guest workspaces:
## no cef-env needed.
wasm-modules:
	cd crates/examples/hello-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/hello-wasm/target/wasm32-unknown-unknown/release/hello_wasm.wasm \
	  -o crates/examples/hello-wasm/component.wasm
	cp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/wasm-host/tests/fixtures/hello.component.wasm
	cp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/hello.component.wasm
	cd crates/examples/hello-wasm-v2 && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/hello-wasm-v2/target/wasm32-unknown-unknown/release/hello_wasm_v2.wasm \
	  -o crates/kernel/host/tests/fixtures/hello-v2.component.wasm

## the drift gate for the committed component artifacts: every copy of the SAME
## module must be byte-identical (bin/node embeds the canonical artifact; the
## kernel test fixtures pin the same bytes). toolchain-independent, so it rides
## the pre-push `test` gate; run `make wasm-modules` to refresh the set.
wasm-modules-check:
	cmp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/wasm-host/tests/fixtures/hello.component.wasm
	cmp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/hello.component.wasm
	@echo "wasm module artifacts are mutually consistent"

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
