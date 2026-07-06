# ducktape build + install entry points.
#
# `make install` builds the networked node and the desktop app, installs
# ducktape-node into ~/.cargo/bin, and places Ducktape.app in /Applications.
# individual targets below for the pieces.

CARGO ?= cargo
BUN ?= bun
APP_DEST ?= /Applications

.PHONY: all dev dogfood-forge node web app sidecar install install-node install-app test clean

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

## stage the daemon as the desktop app's sidecar (app/src-tauri/binaries)
sidecar: app/node_modules
	cd app && $(BUN) run sidecar

## static web bundle -> app/dist
web: app/node_modules
	cd app && $(BUN) run build

## desktop bundle (.app + .dmg under target/release/bundle) — stages the
## sidecar itself via beforeBuildCommand
app: app/node_modules
	cd app && $(BUN) run tauri build

app/node_modules:
	cd app && $(BUN) install

install: install-node install-app

## ducktape-node -> ~/.cargo/bin
install-node:
	$(CARGO) install --path bin/node --locked

## Ducktape.app -> $(APP_DEST)
install-app: app
	mkdir -p "$(APP_DEST)"
	rm -rf "$(APP_DEST)/Ducktape.app"
	cp -R target/release/bundle/macos/Ducktape.app "$(APP_DEST)/"
	@echo "installed $(APP_DEST)/Ducktape.app"

## the full LOCAL verification gate (no hosted CI by design — run this before
## every push): the rust workspace including the process-level e2e suites
## (bin/node spawns a real 4-node cluster over localhost TCP, bin/noded drives
## a real spawned daemon over http/ws), then the app suites with the daemon
## binary staged so the live-daemon wire-parity e2e RUNS instead of skipping,
## and the sim node staged so the provider scenario suite runs too.
test: app/node_modules
	$(CARGO) test --workspace
	$(CARGO) build -p noded -p simnode
	cd app && $(BUN) run typecheck
	cd app && DUCKTAPE_NODED_BIN=$(abspath target/debug/ducktape-noded) DUCKTAPE_SIMNODE_BIN=$(abspath target/debug/ducktape-simnode) $(BUN) run test

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
