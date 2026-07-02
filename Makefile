# ducktape build + install entry points.
#
# `make install` builds the networked node and the desktop app, installs
# ducktape-node into ~/.cargo/bin, and places Ducktape.app in /Applications.
# individual targets below for the pieces.

CARGO ?= cargo
BUN ?= bun
APP_DEST ?= /Applications

.PHONY: all node web app sidecar install install-node install-app test clean

all: node web

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

test:
	$(CARGO) test --workspace
	cd app && $(BUN) run test

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
