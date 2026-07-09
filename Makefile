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

UNAME_S := $(shell uname -s)

.PHONY: all dev dogfood-forge node coordinator coordinator-smoke web app sidecar install install-node install-coordinator install-app stream-types test clean

all: node web

## dev loop: the desktop app + a HOT-RELOADING node. runs `tauri dev` (frontend
## hot-reload) and watches the Rust tree — on any node/kernel change it rebuilds
## ducktape-node and restarts the running node in place, which the app re-adopts.
## see ops/dev.sh. (stop any already-running `tauri dev` first — it owns :1430.)
dev:
	@bash ops/dev.sh

## dogfood: host ducktape's own source in the local dev node's forge module.
## registers a static `ducktape-dev` git remote at the node's forge endpoint and
## pushes `main` (needs a running dev node — `make dev`). see ops/dogfood-forge.sh.
dogfood-forge:
	@bash ops/dogfood-forge.sh

## release build of the networked node (serves the app surface)
node:
	$(CARGO) build --release -p node-bin

## release build of the untrusted UDP coordinator
coordinator:
	$(CARGO) build --release -p coordinator-bin

## coordinator-only verification gate: CLI/policy tests + live UDP smoke
coordinator-smoke:
	$(CARGO) test -p coordinator-bin

## stage the daemon as the desktop app's sidecar (app/src-tauri/binaries)
sidecar: app/node_modules
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
ifeq ($(UNAME_S),Darwin)
app: app/node_modules
	cd app && $(BUN) run tauri build
	bash ops/fix-dmg.sh
else
app: app/node_modules
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
install-node:
	$(CARGO) install --path bin/node --locked

## coordinator -> ~/.cargo/bin/ducktape-coordinator
install-coordinator:
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
stream-types:
	$(CARGO) test -p noded export_ts_bindings

## the full LOCAL verification gate (no hosted CI by design — run this before
## every push): the rust workspace including the process-level e2e suites
## (bin/node spawns a real 4-node cluster over localhost TCP, bin/noded drives
## a real spawned daemon over http/ws), then the app suites with the daemon
## binary staged so the live-daemon wire-parity e2e RUNS instead of skipping,
## and the sim node staged so the provider scenario suite runs too.
test: app/node_modules
	$(CARGO) test --workspace
	$(MAKE) stream-types
	git diff --exit-code -- app/src/domain/stream.gen.ts
	$(CARGO) build -p noded -p simnode
	cd app && $(BUN) run typecheck
	cd app && DUCKTAPE_NODED_BIN=$(abspath target/debug/ducktape-noded) DUCKTAPE_SIMNODE_BIN=$(abspath target/debug/ducktape-simnode) $(BUN) run test

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
