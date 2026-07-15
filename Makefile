# ducktape build + install entry points.
#
# `make install` builds the networked node and native iced desktop app,
# installs ducktape-node, and installs the platform package: Ducktape.app on
# macOS, a self-contained app dir on Linux, or a current-user app plus Start
# menu shortcut on Windows. Every package carries the pinned CEF runtime used
# only by the Browser pane.

CARGO ?= cargo
BUN ?= bun
# Rootless by default on macOS. Operators who deliberately manage a shared
# machine can still override APP_DEST=/Applications themselves.
APP_DEST ?= $(HOME)/Applications
BIN_DEST ?= $(HOME)/.cargo/bin
# Linux: the app's payload directory lives inside ducktape's own home so the
# installed app is self-contained and launcher-spawnable (no LD_LIBRARY_PATH).
DUCKTAPE_HOME ?= $(HOME)/.ducktape

# The native iced shell embeds the pinned CEF distribution only for its Browser
# pane. `cef-env` provisions the CEF build prerequisites and distribution.
# CEF_PATH is where cef-dll-sys downloads the pinned binary distribution. The
# native staging scripts copy the runtime/framework and helpers from there.
CEF_TOOLS ?= $(HOME)/.cache/ducktape-cef-tools
export CEF_PATH ?= $(HOME)/.local/share/cef
# setup.sh drops a ninja binary here on Macs that lack one (cef-dll-sys
# hardcodes the Ninja CMake generator); make sure child builds can see it.
export PATH := $(CEF_TOOLS)/bin:$(PATH)

ifeq ($(OS),Windows_NT)
HOST_OS := Windows
else
HOST_OS := $(shell uname -s)
endif

# The native screen-sharing picker requires macOS 14. Keep the Rust, Swift,
# and C/C++ deployment targets aligned with the bundle's honest minimum.
ifeq ($(HOST_OS),Darwin)
export MACOSX_DEPLOYMENT_TARGET ?= 14.0
endif

.PHONY: all dev demo-seed demo-app demo-clear dogfood-forge node coordinator coordinator-smoke web app macos-smoke macos-cef-smoke sidecar install install-node install-coordinator install-app stream-types test clean cef-env wasm-modules wasm-modules-check

all: app

## provision macOS CEF build prerequisites (ninja); no-op on Linux. idempotent; every
## cargo-touching target depends on it.
cef-env:
	@bash ops/cef-probe/setup.sh "$(CEF_TOOLS)"

## Native iced development app with the matching local node binary. macOS
## runs from a staged .app. Windows runs only through CEF's sandbox-owning
## bootstrap + Rust client DLL pair. Linux can run the flat debug executable.
ifeq ($(HOST_OS),Darwin)
dev: cef-env
	$(CARGO) build -p node-bin
	$(CARGO) build -p ducktape-iced
	bash ops/stage-macos-iced-app.sh debug
	bash ops/check-macos-cef-bundle.sh target/debug/bundle/macos/Ducktape.app
	target/debug/bundle/macos/Ducktape.app/Contents/MacOS/ducktape
else ifeq ($(HOST_OS),Windows)
dev: cef-env
	$(CARGO) build -p node-bin
	$(CARGO) build -p ducktape-iced --lib
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File ops/stage-windows-app.ps1 -Configuration debug -NoArchive
	target/debug/bundle/windows/Ducktape/Ducktape.exe
else
dev: cef-env
	$(CARGO) build -p node-bin
	DUCKTAPE_NODE_BIN="$(abspath target/debug/ducktape-node)" $(CARGO) run -p ducktape-iced
endif

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

## remove the seeded "demo" workspace: stop its node (cmdline-verified pid
## sweep, graceful /v1/shutdown first), delete ~/.ducktape/workspaces/demo, and
## drop it from the registry — other workspaces untouched. See ops/demo-clear.sh.
demo-clear:
	@bash ops/demo-clear.sh

## dogfood: host ducktape's own source in the local dev node's forge module.
## registers a static `ducktape-dev` git remote at the node's forge endpoint and
## synchronizes canonical `origin/dev` into Forge `dev` without moving
## release-only `main`, then verifies the exact ref (needs a running dev node —
## `make dev`). see ops/dogfood-forge.sh.
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

## build the daemon that is staged beside the desktop executable.
sidecar: cef-env
	$(CARGO) build -p node-bin

## static web bundle -> app/dist
web: app/node_modules
	cd app && $(BUN) run build

## Desktop build — stages the node beside the iced executable. macOS gets an
## app bundle + zip, Windows a relocatable directory + zip, and Linux a
## relocatable directory + tarball. Native staging keeps CEF framework/runtime
## registration coupled to the executable without an external bundler.
ifeq ($(HOST_OS),Darwin)
app: cef-env
	$(CARGO) build --release -p ducktape-iced -p node-bin
	bash ops/stage-macos-iced-app.sh
	bash ops/check-macos-cef-bundle.sh target/release/bundle/macos/Ducktape.app
else ifeq ($(HOST_OS),Windows)
app: cef-env
	$(CARGO) build --release -p node-bin
	$(CARGO) build --release -p ducktape-iced --lib
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File ops/stage-windows-app.ps1
else
app: cef-env
	$(CARGO) build --release -p ducktape-iced -p node-bin
	bash ops/stage-linux-app.sh
endif

## Build and exercise the real staged macOS window, including close-to-menu-bar
## and Dock/Finder activation reopen. Requires Accessibility permission for the
## invoking terminal because it inspects and presses native AppKit controls.
ifeq ($(HOST_OS),Darwin)
macos-smoke: app
	bash ops/smoke-macos-iced-app.sh target/release/bundle/macos/Ducktape.app

macos-cef-smoke: cef-env
	$(CARGO) build -p ducktape-iced --bin cef-probe
	$(CARGO) build -p node-bin
	DUCKTAPE_MACOS_BINARY=cef-probe bash ops/stage-macos-iced-app.sh debug
	bash ops/check-macos-cef-bundle.sh target/debug/bundle/macos/Ducktape.app
	bash ops/smoke-macos-cef-probe.sh target/debug/bundle/macos/Ducktape.app
else
macos-smoke:
	@echo "macos-smoke must run on macOS" >&2
	@exit 2

macos-cef-smoke:
	@echo "macos-cef-smoke must run on macOS" >&2
	@exit 2
endif

# re-run bun install whenever the manifest or lockfile changes, not just when
# node_modules is absent; the touch keeps the dir newer than its prerequisites
# (bun does not reliably update the dir mtime when nothing needs fetching).
app/node_modules: app/package.json app/bun.lock
	cd app && $(BUN) install --frozen-lockfile
	touch app/node_modules

install: install-app

## Optional operator CLI install. The desktop bundle already carries the exact
## matching node sidecar and does not depend on this PATH copy.
install-node: cef-env
	$(CARGO) install --path bin/node --locked

## coordinator -> ~/.cargo/bin/ducktape-coordinator
install-coordinator: cef-env
	$(CARGO) build --release -p coordinator-bin
	mkdir -p "$(BIN_DEST)"
	install -m 755 target/release/coordinator "$(BIN_DEST)/ducktape-coordinator"

## macOS: Ducktape.app -> $(APP_DEST); Windows: current-user LocalAppData plus
## a Start-menu shortcut; Linux: the staged self-contained dir ->
## $(DUCKTAPE_HOME)/app (binary + ducktape-node sidecar + pinned CEF
## runtime in ONE directory, so sidecar sibling-resolution and the DT_RPATH
## $ORIGIN lookup both land beside the executable), plus a launcher symlink
## in $(BIN_DEST) — a symlink, NOT a copy: ld.so resolves $ORIGIN through
## symlinks to the real file's directory, while a copied binary would sit
## beside no runtime and fall back to LD_LIBRARY_PATH, which is how a system
## CEF of the wrong major version silently breaks IME.
ifeq ($(HOST_OS),Darwin)
install-app: app
	mkdir -p "$(APP_DEST)"
	@if [ -L "$(DUCKTAPE_HOME)" ]; then echo "refusing symbolic-link state root: $(DUCKTAPE_HOME)" >&2; exit 1; fi
	@if [ -d "$(DUCKTAPE_HOME)" ]; then chmod 700 "$(DUCKTAPE_HOME)"; fi
	rm -rf "$(APP_DEST)/Ducktape.app"
	cp -R target/release/bundle/macos/Ducktape.app "$(APP_DEST)/"
	bash ops/check-macos-cef-bundle.sh "$(APP_DEST)/Ducktape.app"
	@echo "installed $(APP_DEST)/Ducktape.app"
else ifeq ($(HOST_OS),Windows)
install-app: app
	powershell.exe -NoProfile -ExecutionPolicy Bypass -File ops/stage-windows-app.ps1 -Install
else
install-app: app
	@if [ -L "$(DUCKTAPE_HOME)" ]; then echo "refusing symbolic-link state root: $(DUCKTAPE_HOME)" >&2; exit 1; fi
	mkdir -p "$(DUCKTAPE_HOME)"
	chmod 700 "$(DUCKTAPE_HOME)"
	rm -rf "$(DUCKTAPE_HOME)/app"
	cp -a target/release/bundle/linux/ducktape "$(DUCKTAPE_HOME)/app"
	mkdir -p "$(BIN_DEST)"
	ln -sfn "$(DUCKTAPE_HOME)/app/ducktape" "$(BIN_DEST)/ducktape"
	@echo "installed $(DUCKTAPE_HOME)/app ($(BIN_DEST)/ducktape -> app/ducktape)"
	bash ops/install-desktop-entry.sh "$(DUCKTAPE_HOME)/app/ducktape"
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
	cd crates/examples/directory-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/directory-wasm/target/wasm32-unknown-unknown/release/directory_wasm.wasm \
	  -o crates/examples/directory-wasm/component.wasm
	cp crates/examples/directory-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/directory.component.wasm
	cd crates/examples/sibling-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/sibling-wasm/target/wasm32-unknown-unknown/release/sibling_wasm.wasm \
	  -o crates/kernel/wasm-host/tests/fixtures/sibling.component.wasm
	cd crates/examples/vaults-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/vaults-wasm/target/wasm32-unknown-unknown/release/vaults_wasm.wasm \
	  -o crates/examples/vaults-wasm/component.wasm
	cp crates/examples/vaults-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/vaults.component.wasm

	cd crates/examples/jobs-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/jobs-wasm/target/wasm32-unknown-unknown/release/jobs_wasm.wasm \
	  -o crates/examples/jobs-wasm/component.wasm
	cp crates/examples/jobs-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/jobs.component.wasm

	cd crates/examples/inbox-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/inbox-wasm/target/wasm32-unknown-unknown/release/inbox_wasm.wasm \
	  -o crates/examples/inbox-wasm/component.wasm
	cp crates/examples/inbox-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/inbox.component.wasm

	cd crates/examples/tasks-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/tasks-wasm/target/wasm32-unknown-unknown/release/tasks_wasm.wasm \
	  -o crates/examples/tasks-wasm/component.wasm
	cp crates/examples/tasks-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/tasks.component.wasm

	cd crates/examples/tagging-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/tagging-wasm/target/wasm32-unknown-unknown/release/tagging_wasm.wasm \
	  -o crates/examples/tagging-wasm/component.wasm
	cp crates/examples/tagging-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/tagging.component.wasm

	cd crates/examples/capability-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/capability-wasm/target/wasm32-unknown-unknown/release/capability_wasm.wasm \
	  -o crates/examples/capability-wasm/component.wasm
	cp crates/examples/capability-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/capability.component.wasm

	cd crates/examples/duckdns-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/duckdns-wasm/target/wasm32-unknown-unknown/release/duckdns_wasm.wasm \
	  -o crates/examples/duckdns-wasm/component.wasm
	cp crates/examples/duckdns-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/duckdns.component.wasm

	cd crates/examples/identity-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/identity-wasm/target/wasm32-unknown-unknown/release/identity_wasm.wasm \
	  -o crates/examples/identity-wasm/component.wasm
	cp crates/examples/identity-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/identity.component.wasm

	cd crates/examples/gateway-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/gateway-wasm/target/wasm32-unknown-unknown/release/gateway_wasm.wasm \
	  -o crates/examples/gateway-wasm/component.wasm
	cp crates/examples/gateway-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/gateway.component.wasm

	cd crates/examples/governance-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/governance-wasm/target/wasm32-unknown-unknown/release/governance_wasm.wasm \
	  -o crates/examples/governance-wasm/component.wasm
	cp crates/examples/governance-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/governance.component.wasm

	cd crates/examples/pages-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/pages-wasm/target/wasm32-unknown-unknown/release/pages_wasm.wasm \
	  -o crates/examples/pages-wasm/component.wasm
	cp crates/examples/pages-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/pages.component.wasm

	cd crates/examples/chat-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/chat-wasm/target/wasm32-unknown-unknown/release/chat_wasm.wasm \
	  -o crates/examples/chat-wasm/component.wasm
	cp crates/examples/chat-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/chat.component.wasm

	cd crates/examples/saga-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/saga-wasm/target/wasm32-unknown-unknown/release/saga_wasm.wasm \
	  -o crates/examples/saga-wasm/component.wasm
	cp crates/examples/saga-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/saga.component.wasm

	cd crates/examples/agent-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/agent-wasm/target/wasm32-unknown-unknown/release/agent_wasm.wasm \
	  -o crates/examples/agent-wasm/component.wasm
	cp crates/examples/agent-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/agent.component.wasm

	cd crates/examples/automations-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/automations-wasm/target/wasm32-unknown-unknown/release/automations_wasm.wasm \
	  -o crates/examples/automations-wasm/component.wasm
	cp crates/examples/automations-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/automations.component.wasm

	cd crates/examples/runs-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/examples/runs-wasm/target/wasm32-unknown-unknown/release/runs_wasm.wasm \
	  -o crates/examples/runs-wasm/component.wasm
	cp crates/examples/runs-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/runs.component.wasm

## the drift gate for the committed component artifacts: every copy of the SAME
## module must be byte-identical (bin/node embeds the canonical artifact; the
## kernel test fixtures pin the same bytes). toolchain-independent, so it rides
## the pre-push `test` gate; run `make wasm-modules` to refresh the set.
wasm-modules-check:
	cmp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/wasm-host/tests/fixtures/hello.component.wasm
	cmp crates/examples/hello-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/hello.component.wasm
	cmp crates/examples/directory-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/directory.component.wasm
	cmp crates/examples/vaults-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/vaults.component.wasm
	cmp crates/examples/jobs-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/jobs.component.wasm
	cmp crates/examples/inbox-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/inbox.component.wasm
	cmp crates/examples/tasks-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/tasks.component.wasm
	cmp crates/examples/tagging-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/tagging.component.wasm
	cmp crates/examples/capability-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/capability.component.wasm
	cmp crates/examples/duckdns-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/duckdns.component.wasm
	cmp crates/examples/identity-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/identity.component.wasm
	cmp crates/examples/gateway-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/gateway.component.wasm
	cmp crates/examples/governance-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/governance.component.wasm
	cmp crates/examples/pages-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/pages.component.wasm
	cmp crates/examples/chat-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/chat.component.wasm
	cmp crates/examples/saga-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/saga.component.wasm
	cmp crates/examples/agent-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/agent.component.wasm
	cmp crates/examples/automations-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/automations.component.wasm
	cmp crates/examples/runs-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/runs.component.wasm
	@echo "wasm module artifacts are mutually consistent"

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules
