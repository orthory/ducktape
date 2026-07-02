# ducktape build + install entry points.
#
# `make install` builds the node daemon and the desktop app, installs
# ducktape-noded into ~/.cargo/bin, and places Ducktape.app in /Applications.
# individual targets below for the pieces.

CARGO ?= cargo
BUN ?= bun
APP_DEST ?= /Applications

.PHONY: all noded web app sidecar install install-noded install-app test clean

all: noded web

## release build of the node daemon
noded:
	$(CARGO) build --release -p noded

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

install: install-noded install-app

## ducktape-noded -> ~/.cargo/bin
install-noded:
	$(CARGO) install --path bin/noded --locked

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
