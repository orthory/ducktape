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

# The desktop shell runs on the standalone tauri-runtime-cef crate
# (github.com/byeongsu-hong/tauri-runtime-cef) against published crates.io
# tauri: plain cargo builds work everywhere. `cef-env` (idempotent) only
# provisions macOS bundling prerequisites — ninja + the pinned feat/cef CLI
# checkout, see ops/cef-probe/setup.sh; on Linux it exits immediately.
# CEF_PATH is where the CEF binary distribution
# lives (cef-dll-sys downloads into it on first build; the tauri CLI hands
# it to the macOS bundler for the framework/helper copy).
CEF_CLONE ?= $(HOME)/.cache/ducktape-cef-probe/tauri-cef
export CEF_PATH ?= $(HOME)/.local/share/cef
# setup.sh drops a ninja binary here on Macs that lack one (cef-dll-sys
# hardcodes the Ninja CMake generator); make sure child builds can see it.
export PATH := $(dir $(CEF_CLONE))bin:$(PATH)

UNAME_S := $(shell uname -s)

.PHONY: all dev demo-seed demo-app dogfood-forge node coordinator coordinator-smoke web app sidecar install install-node install-coordinator install-app stream-types test clean cef-env

all: node web

## provision macOS bundling prerequisites (ninja + the pinned upstream
## feat/cef tauri CLI checkout); no-op on Linux. idempotent; every
## cargo-touching target depends on it.
cef-env:
	@bash ops/cef-probe/setup.sh "$(CEF_CLONE)"

## dev loop: the desktop app + a HOT-RELOADING node. runs `tauri dev` (frontend
## hot-reload) and watches the Rust tree — on any node/kernel change it rebuilds
## ducktape-node and restarts the running node in place, which the app re-adopts.
## see ops/dev.sh. (stop any already-running `tauri dev` first — it owns :1430.)
dev: cef-env
	@bash ops/dev.sh

## seed a local "demo" network preloaded with sample data — chat channels +
## messages, a tasks board, pages, a registered agent (with a live @mention run),
## jobs, an inbox note, an automation rule — plus TWO gateway web-app routes: a
## NETWORK-hosted static site (DuckFS) and a USER-hosted loopback app. Registers a
## "demo" workspace in ~/.ducktape and makes it active; just open the app. Builds
## ducktape-node if needed (or set DUCKTAPE_NODE_BIN). See ops/demo-seed.sh.
demo-seed: cef-env
	@bash ops/demo-seed.sh

## serve the user-hosted web app behind the demo's app.<id>.duck gateway route
## (demo-seed publishes the route; this runs the loopback server it proxies to).
## Foreground — Ctrl-C to stop. See ops/demo-app.sh.
demo-app:
	@bash ops/demo-app.sh

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
## --ignore-version-mismatches: the CLI compares the tauri crate version to
## @tauri-apps/api, and the pinned feat/cef checkout's CLI carries an
## unreleased dev version — the mismatch is a false positive, the real base
## is 2.11.x on both sides.
ifeq ($(UNAME_S),Darwin)
app: cef-env app/node_modules
	cd app && $(CARGO) run --manifest-path "$(CEF_CLONE)/crates/tauri-cli/Cargo.toml" --bin cargo-tauri -- build --ignore-version-mismatches
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
test: cef-env app/node_modules
	$(CARGO) test --workspace
	$(MAKE) stream-types
	git diff --exit-code -- app/src/domain/stream.gen.ts
	$(CARGO) build -p noded -p simnode
	cd app && $(BUN) run typecheck
	cd app && DUCKTAPE_NODED_BIN=$(abspath target/debug/ducktape-noded) DUCKTAPE_SIMNODE_BIN=$(abspath target/debug/ducktape-simnode) $(BUN) run test

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
