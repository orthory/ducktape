# ducktape build + install entry points.
#
# `make install` builds the networked node and the desktop app, installs
# ducktape-node into ~/.cargo/bin, and installs the app — on macOS
# Ducktape.app into /Applications, on Linux the self-contained app dir
# (binary + ducktape-node sidecar + pinned CEF runtime, resolved via the
# binary's DT_RPATH of $ORIGIN) into ~/.ducktape/app with a `ducktape`
# launcher symlink in ~/.cargo/bin. individual targets below for the pieces.

CARGO ?= cargo
BUN ?= bun
BUILD_WITH ?= $(CURDIR)/ops/build-with.sh
APP_DEST ?= /Applications
BIN_DEST ?= $(HOME)/.cargo/bin
# Linux: the app's payload directory lives inside ducktape's own home so the
# installed app is self-contained and launcher-spawnable (no LD_LIBRARY_PATH).
DUCKTAPE_HOME ?= $(HOME)/.ducktape

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

.PHONY: all build-tools dev demo-seed demo-app demo-clear dogfood-forge node coordinator coordinator-smoke web app sidecar install install-node install-coordinator install-app stream-types test clean cef-env wasm-modules wasm-modules-check

all: node web

## report the optional Rust build accelerators detected on this host. Makefile
## build entry points use sccache when installed and mold+clang on Linux; both
## fall back cleanly, so neither is a prerequisite.
build-tools:
	@$(BUILD_WITH) --status

## provision macOS bundling prerequisites (ninja + the pinned upstream
## feat/cef tauri CLI checkout); no-op on Linux. idempotent; every
## cargo-touching target depends on it.
cef-env:
	@bash ops/cef-probe/setup.sh "$(CEF_CLONE)"

## dev loop: the desktop app + a HOT-RELOADING node. runs `tauri dev` (frontend
## hot-reload) and watches the Rust tree — on any node/kernel change it rebuilds
## ducktape-node and restarts the running node in place, which the app re-adopts.
## see ops/dev.sh. (stop any already-running `tauri dev` first — it owns :1430.)
dev: cef-env app/node_modules
	@$(BUILD_WITH) bash ops/dev.sh

## seed a local "demo" network preloaded with sample data — chat channels +
## messages, a tasks board, pages, a registered agent (with a live @mention run),
## jobs, an inbox note, an automation rule — plus TWO gateway web-app routes: a
## NETWORK-hosted static site (DuckFS) and a USER-hosted loopback app. Registers a
## "demo" workspace in ~/.ducktape and makes it active; just open the app. Builds
## ducktape-node if needed (or set DUCKTAPE_NODE_BIN). See ops/demo-seed.sh.
demo-seed: cef-env
	@$(BUILD_WITH) bash ops/demo-seed.sh

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
## fetches + pushes canonical `origin/dev` to Forge `main`, then verifies the
## exact ref (needs a running dev node — `make dev`). see ops/dogfood-forge.sh.
dogfood-forge:
	@bash ops/dogfood-forge.sh

## release build of the networked node (serves the app surface)
node: cef-env
	$(BUILD_WITH) $(CARGO) build --release -p node-bin

## release build of the untrusted UDP coordinator
coordinator: cef-env
	$(BUILD_WITH) $(CARGO) build --release -p coordinator-bin

## coordinator-only verification gate: CLI/policy tests + live UDP smoke
coordinator-smoke: cef-env
	$(BUILD_WITH) $(CARGO) test -p coordinator-bin

## stage the daemon as the desktop app's sidecar (app/src-tauri/binaries)
sidecar: cef-env app/node_modules
	cd app && $(BUILD_WITH) $(BUN) run sidecar

## static web bundle -> app/dist
web: app/node_modules
	cd app && $(BUN) run build

## desktop build — stages the sidecar itself via beforeBuildCommand. on macOS
## a bundle (.app + .dmg under target/release/bundle); on Linux a relocatable
## self-contained dir + release tarball under target/release/bundle/linux
## (ops/stage-linux-app.sh; --no-bundle because tauri's deb/rpm/appimage
## packagers know nothing about the CEF payload — the staging script is the
## Linux bundler). the dmg post-fix hides .VolumeIcon.icns, which macOS 26
## Finder would otherwise show overlapping the app icon — see ops/fix-dmg.sh.
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
	cd app && $(BUILD_WITH) $(CARGO) run --manifest-path "$(CEF_CLONE)/crates/tauri-cli/Cargo.toml" --bin cargo-tauri -- build --ignore-version-mismatches
	bash ops/check-macos-cef-bundle.sh target/release/bundle/macos/Ducktape.app
	bash ops/smoke-macos-app.sh target/release/bundle/macos/Ducktape.app
	bash ops/fix-dmg.sh
else
app: cef-env app/node_modules
	cd app && $(BUILD_WITH) $(BUN) run tauri build --no-bundle
	bash ops/stage-linux-app.sh
endif

# re-run bun install whenever the manifest or lockfile changes, not just when
# node_modules is absent; the touch keeps the dir newer than its prerequisites
# (bun does not reliably update the dir mtime when nothing needs fetching).
app/node_modules: app/package.json app/bun.lock
	cd app && $(BUN) install --frozen-lockfile
	touch app/node_modules

install: install-node install-app

## ducktape-node -> ~/.cargo/bin
install-node: cef-env
	$(BUILD_WITH) $(CARGO) install --path bin/node --locked

## coordinator -> ~/.cargo/bin/ducktape-coordinator
install-coordinator: cef-env
	$(BUILD_WITH) $(CARGO) build --release -p coordinator-bin
	mkdir -p "$(BIN_DEST)"
	install -m 755 target/release/coordinator "$(BIN_DEST)/ducktape-coordinator"

## macOS: Ducktape.app -> $(APP_DEST); Linux: the staged self-contained dir
## -> $(DUCKTAPE_HOME)/app (binary + ducktape-node sidecar + pinned CEF
## runtime in ONE directory, so sidecar sibling-resolution and the DT_RPATH
## $ORIGIN lookup both land beside the executable), plus a launcher symlink
## in $(BIN_DEST) — a symlink, NOT a copy: ld.so resolves $ORIGIN through
## symlinks to the real file's directory, while a copied binary would sit
## beside no runtime and fall back to LD_LIBRARY_PATH, which is how a system
## CEF of the wrong major version silently breaks IME.
ifeq ($(UNAME_S),Darwin)
install-app: app
	mkdir -p "$(APP_DEST)"
	rm -rf "$(APP_DEST)/Ducktape.app"
	cp -R target/release/bundle/macos/Ducktape.app "$(APP_DEST)/"
	bash ops/check-macos-cef-bundle.sh "$(APP_DEST)/Ducktape.app"
	@echo "installed $(APP_DEST)/Ducktape.app"
else
install-app: app
	mkdir -p "$(DUCKTAPE_HOME)"
	rm -rf "$(DUCKTAPE_HOME)/app"
	cp -a target/release/bundle/linux/ducktape "$(DUCKTAPE_HOME)/app"
	mkdir -p "$(BIN_DEST)"
	ln -sfn "$(DUCKTAPE_HOME)/app/ducktape" "$(BIN_DEST)/ducktape"
	@echo "installed $(DUCKTAPE_HOME)/app ($(BIN_DEST)/ducktape -> app/ducktape)"
	bash ops/install-desktop-entry.sh "$(DUCKTAPE_HOME)/app/ducktape"
endif

## regenerate app/src/domain/stream.gen.ts from the stream contract
stream-types: cef-env
	$(BUILD_WITH) $(CARGO) test -p noded export_ts_bindings

## the full LOCAL verification gate (no hosted CI by design — run this before
## every push): the rust workspace including the process-level e2e suites
## (bin/node spawns a real 4-node cluster over localhost TCP, bin/noded drives
## a real spawned daemon over http/ws), then the app suites with the daemon
## binary staged so the live-daemon wire-parity e2e RUNS instead of skipping,
## and the sim node staged so the provider scenario suite runs too.
test: cef-env app/node_modules wasm-modules-check
	$(BUILD_WITH) $(CARGO) test --workspace
	$(MAKE) stream-types
	git diff --exit-code -- app/src/domain/stream.gen.ts
	$(BUILD_WITH) $(CARGO) build -p noded -p simnode
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
	@echo "wasm module artifacts are mutually consistent"

clean:
	$(CARGO) clean
	rm -rf app/dist app/node_modules app/src-tauri/binaries
