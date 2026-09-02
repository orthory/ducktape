# ducktape build + install entry points.
#
# `make node` / `make coordinator` build the runnable product surfaces (the
# networked node daemon and the untrusted UDP coordinator). `make install`
# installs the `ducktape` operator CLI and, on macOS, Ducktape.app. `make test`
# is the full local verification gate — run it before every push.

CARGO ?= cargo
APP_DEST ?= $(HOME)/Applications
BIN_DEST ?= $(HOME)/.cargo/bin
UNAME_S := $(shell uname -s)

.PHONY: all app dev dev-clear demo-seed demo-app demo-clear dogfood-forge node coordinator coordinator-smoke install install-app install-node install-coordinator test clean wasm-modules wasm-modules-check wasm-repro-check labs-gate

## build every workspace crate (the default target)
all:
	$(CARGO) build --workspace

## the app dev loop: seed the "demo" localnet if it does not exist yet
## (DEV_RESEED=1 forces a fresh seed), start its node when it is not already
## serving, start the local compute/agent/airlock services, sync ducktape's own
## repo into that node's forge (dogfood-forge — non-fatal when origin is
## unreachable), then run the desktop app against it in the foreground. Ctrl-C
## quits the app and leaves the node and services running; `make dev-clear`
## stops that background runtime without deleting its state, while
## `make demo-clear` removes the workspace entirely.
dev:
	@bash ops/dev.sh

## stop the demo node and compute/agent/airlock services left by `make dev`.
## Preserves the workspace, registry entry, module state, wallets, and
## credentials. The foreground app and `make demo-app` are not killed.
dev-clear:
	@bash ops/dev-clear.sh

## seed a local "demo" network preloaded with sample data — chat channels +
## messages, a tasks board, pages, a registered agent (with a live @mention run),
## jobs, an inbox note, an automation rule — plus TWO gateway web-app routes: a
## NETWORK-hosted static site (DuckFS) and a USER-hosted loopback app. Registers a
## "demo" workspace in ~/.ducktape and makes it active. Builds ducktape if needed
## (or set DUCKTAPE_NODE_BIN). See ops/demo-seed.sh.
demo-seed:
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
## release-only `main`, then verifies the exact ref (needs a running dev node).
## see ops/dogfood-forge.sh.
dogfood-forge:
	@bash ops/dogfood-forge.sh

## build-check the quarantined labs crate. It is EXCLUDED from the workspace
## (its own Cargo.lock) so its revm/alloy dep tree never taxes workspace gates;
## this target is how CI/devs still keep it compiling.
labs-gate:
	$(CARGO) check --manifest-path crates/labs/Cargo.toml

## release build of the networked node (the app-facing daemon surface)
node:
	$(CARGO) build --release -p node-bin

## release build of the untrusted UDP coordinator
coordinator:
	$(CARGO) build --release -p coordinator-bin

## coordinator-only verification gate: CLI/policy tests + live UDP smoke
coordinator-smoke:
	$(CARGO) test -p coordinator-bin

ifeq ($(UNAME_S),Darwin)
# Build cargo-ice from the same ducktape-ui rev as the app. A global cargo-ice
# can parse a different language than the compiler in app/Cargo.toml.
#
# Both the URL and the rev come from app/Cargo.toml, so the install source
# cannot drift from the pin the app compiles against.
#
# This installs straight from the pinned rev. `cargo install --git` resolves the
# package's whole workspace, so it also clones the one git dependency no part of
# cargo-ice uses (pornin/ecgfp5, which the trading example wants). That clone is
# the deliberate price: the alternative was a hand-maintained `ice-install/<rev>`
# branch holding the same rev minus the example members, which had to be rebased
# and pushed on every pin bump and broke `make app` with a bare git exit 128
# every time someone forgot.
ICE_GIT = $(shell sed -n 's|.*git = "\([^"]*ducktape-ui.git\)", rev = .*|\1|p' app/Cargo.toml | head -n1)
ICE_REV = $(shell sed -n 's/.*ducktape-ui.git", rev = "\([^"]*\)".*/\1/p' app/Cargo.toml | head -n1)
ICE_ROOT = $(CURDIR)/target/cargo-ice/$(ICE_REV)
ICE_BIN = $(ICE_ROOT)/bin/cargo-ice

$(ICE_BIN):
	CARGO_TARGET_DIR="$(CURDIR)/target/cargo-ice-build" $(CARGO) install cargo-ice \
		--git "$(ICE_GIT)" --rev "$(ICE_REV)" --locked --root "$(ICE_ROOT)"

## build the signed-ad-hoc Ducktape.app and DMG under target/ice-bundle
app: $(ICE_BIN)
	"$(ICE_BIN)" bundle -p ducktape-app

## install the operator CLI and desktop app without requiring root
install: install-node install-app

install-app: app
	mkdir -p "$(APP_DEST)"
	rm -rf "$(APP_DEST)/Ducktape.app"
	cp -R target/ice-bundle/Ducktape.app "$(APP_DEST)/"
	@echo "installed $(APP_DEST)/Ducktape.app"
else
## install the ducktape operator CLI into ~/.cargo/bin
install: install-node

install-app:
	@echo "install-app is currently supported on macOS" >&2
	@exit 2
endif

## the binary embeds no wasm: `node init` founds a network from a directory of
## <id>.component.wasm, so installing the node also installs that set.
install-node:
	$(CARGO) install --path bin/node --locked
	mkdir -p "$${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}"
	@for m in $(BUILDER_MODULES); do \
	  id=$$(basename $$m) && \
	  cp $$m/component.wasm "$${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}/$$id.component.wasm" || exit 1; \
	done
	@echo "installed module components into $${DUCKTAPE_MODULES_DIR:-$$HOME/.ducktape/modules}"

## coordinator -> ~/.cargo/bin/ducktape-coordinator
install-coordinator:
	$(CARGO) build --release -p coordinator-bin
	mkdir -p "$(BIN_DEST)"
	install -m 755 target/release/coordinator "$(BIN_DEST)/ducktape-coordinator"

## the full LOCAL verification gate (no hosted CI by design — run this before
## every push): the wasm-artifact drift gate, the rust workspace including the
## process-level e2e suites (bin/node spawns a real 4-node cluster over localhost
## TCP, bin/noded drives a real spawned daemon over http/ws), the consensus
## sim-feature suite, and a build of the noded + simnode binaries the test
## harnesses stage.
# WHERE THE E2E SUITES PUT NODE STORAGE — pinned to disk, on purpose.
#
# Every spawned cluster writes `storage=$TMPDIR/.tmpXXXX/storage-N`, and a test
# that panics (or a node killed with it) leaves the whole tree behind. On a host
# where /tmp is tmpfs — the default on this dev box — those leaks are RAM. One
# session left 11 dirs totalling 22 GB, two of them 7.2 GB each, and since
# tmpfs has no swap the box lost that memory for good: each gate run started
# with less than the last, until rustc and ld began dying mid-compile. The runs
# measuring the box were degrading it.
#
# Pinning TMPDIR under target/ makes a leak cost disk instead of memory, and the
# rm -rf reclaims the previous run's leftovers before each pass. The leak itself
# is still a bug worth fixing in the harnesses — see #887.
TEST_TMPDIR := $(CURDIR)/target/test-tmp

test: wasm-modules-check
# The reclaim may not fail the gate: a first run has nothing to clean.
	-rm -rf "$(TEST_TMPDIR)" 2>/dev/null
	mkdir -p "$(TEST_TMPDIR)"
	TMPDIR="$(TEST_TMPDIR)" $(CARGO) test --workspace
# the auth page's pure helpers (fragment parsing, DER→raw, SPKI→SEC1) — the
# browser half of `crates/authpage`'s contract, dependency-free under node.
	node ops/auth-page/test.mjs
# demo-clear's refusal line against a stub admin surface: the reason token it
# prints has to be the node's own, not one invented in the script. Needs `bun`
# (so does demo-clear itself); the script skips with a notice where there is
# none, like the podman lines above.
	bash ops/demo-clear-test.sh
# the #[ignore]d tests are ignored ONLY because they must not share a process
# with the parallel suite — they still have to run. `absolute_configs_resolve_
# after_launch_cwd_is_deleted` re-execs the test binary, and doing that under 32
# live libtest threads made unrelated tests fail ~4 runs in 11 with integrity
# errors. Serial + its own invocation is the isolation. See #887.
	TMPDIR="$(TEST_TMPDIR)" $(CARGO) test -p node-bin --bin ducktape -- --ignored --test-threads=1
	TMPDIR="$(TEST_TMPDIR)" $(CARGO) test -p consensus --features sim
# ducktape-app rides this line for a reason: `cargo test` builds the TEST
# target, which links dev-dependencies, so a product path reaching for a
# dev-only crate compiles under every test lane and breaks only the BINARY.
# That is not hypothetical — it shipped and sat on dev for 81 commits.
	$(CARGO) build -p noded-bin -p simnode -p ducktape-app

## rebuild every wasm guest module into its componentized artifact and refresh
## EVERY committed copy in one sweep (the canonical node-embedded artifact +
## the kernel test fixtures), so the copies can never drift apart. requires
## the wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)
## and wasm-tools (cargo install wasm-tools). component bytes are toolchain-
## dependent: a rebuild on a different rustc may legitimately differ from the
## committed bytes — commit the refreshed set TOGETHER; `wasm-modules-check`
## guards mutual consistency. bytes no longer depend on WHERE the checkout
## lives (guest-builder remaps the path prefixes away), so the same toolchain
## on any box reproduces them — `wasm-repro-check` is that pin. nor on WHEN:
## guest-builder seeds the scratch workspace from the host `Cargo.lock`, so a
## crates.io publish no longer moves these bytes — a `Cargo.lock` bump that
## touches a guest-graph crate does, and that is a reviewable diff.
#
# Every product/example module carries its own guest port (src/guest.rs behind
# the `guest` feature); guest-builder synthesizes the packaging workspace and
# writes the canonical component.wasm into the module directory itself. The
# four kernel-fixture test guests (hello, hello-replacement, sibling, object) keep
# their standalone crates/guests workspaces below.
BUILDER_MODULES := \
  crates/examples/directory \
  crates/modules/apps/inbox crates/modules/apps/pages crates/modules/apps/agent \
  crates/modules/apps/automations crates/modules/apps/runs \
  crates/modules/apps/tasks crates/modules/apps/chat crates/modules/apps/files \
  crates/modules/apps/vaults crates/modules/apps/forge \
  crates/modules/system/tagging crates/modules/system/dispatch \
  crates/modules/system/capability crates/modules/system/identity \
  crates/modules/system/gateway crates/modules/system/governance \
  crates/modules/system/saga crates/modules/system/acl crates/modules/system/kv

# Modules that additionally ship an INDEX guest (src/index_guest.rs behind the
# `index-guest` feature): guest-builder --index writes the canonical
# index.wasm (core wasm, no componentize) into the module directory, which
# noded embeds via include_bytes!. The reference testmap mapper is the
# indexer crate's test fixture and rides the same sweep.
INDEX_MODULES := \
  crates/modules/apps/chat crates/modules/apps/tasks crates/modules/apps/pages \
  crates/modules/apps/inbox crates/modules/system/saga \
  crates/kernel/index-guest/testmap

# The netstack guest: the reachability machine as a `ducktape:netstack`
# component (crates/networking/netstack-machine/src/guest.rs behind the same
# `guest` feature convention). Not a consensus module, so no kernel fixture
# copy: bin/node embeds the artifact from the crate directory and the
# netstack-wasm scenario lane reads it from there.
NETSTACK_GUEST := crates/networking/netstack-machine

wasm-modules:
	@for m in $(BUILDER_MODULES); do \
	  id=$$(basename $$m) && \
	  $(CARGO) run -q -p guest-builder -- $$m && \
	  cp $$m/component.wasm \
	    crates/kernel/host/tests/fixtures/$$id.component.wasm || exit 1; \
	done
	@for m in $(INDEX_MODULES); do \
	  $(CARGO) run -q -p guest-builder -- --index $$m || exit 1; \
	done
	$(CARGO) run -q -p guest-builder -- $(NETSTACK_GUEST)
	# hello mirrors its component into BOTH fixture homes; sibling/object write
	# straight to the wasm-host fixture with no guest copy; hello-replacement
	# builds the replacement crate directly into the host fixture. Each shape is
	# unique — kept explicit.
	cd crates/guests/hello-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/guests/hello-wasm/target/wasm32-unknown-unknown/release/hello_wasm.wasm \
	  -o crates/guests/hello-wasm/component.wasm
	cp crates/guests/hello-wasm/component.wasm \
	  crates/kernel/wasm-host/tests/fixtures/hello.component.wasm
	cp crates/guests/hello-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/hello.component.wasm
	cd crates/guests/hello-wasm-replacement && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/guests/hello-wasm-replacement/target/wasm32-unknown-unknown/release/hello_wasm_replacement.wasm \
	  -o crates/kernel/host/tests/fixtures/hello-replacement.component.wasm
	cd crates/guests/sibling-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/guests/sibling-wasm/target/wasm32-unknown-unknown/release/sibling_wasm.wasm \
	  -o crates/kernel/wasm-host/tests/fixtures/sibling.component.wasm
	cd crates/guests/object-wasm && $(CARGO) build --target wasm32-unknown-unknown --release
	wasm-tools component new \
	  crates/guests/object-wasm/target/wasm32-unknown-unknown/release/object_wasm.wasm \
	  -o crates/kernel/wasm-host/tests/fixtures/object.component.wasm

## the drift gate for the committed component artifacts: every copy of the SAME
## module must be byte-identical (`node init` hashes the bundle into the
## descriptor; the kernel test fixtures pin the same bytes).
## toolchain-independent, so it rides the pre-push `test` gate; run
## `make wasm-modules` to refresh the set.
wasm-modules-check:
	cmp crates/guests/hello-wasm/component.wasm \
	  crates/kernel/wasm-host/tests/fixtures/hello.component.wasm
	cmp crates/guests/hello-wasm/component.wasm \
	  crates/kernel/host/tests/fixtures/hello.component.wasm
	@for m in $(BUILDER_MODULES); do \
	  id=$$(basename $$m) && \
	  cmp $$m/component.wasm \
	    crates/kernel/host/tests/fixtures/$$id.component.wasm || exit 1; \
	done
	@for m in $(INDEX_MODULES); do \
	  test -f $$m/index.wasm || { echo "missing $$m/index.wasm (make wasm-modules)"; exit 1; }; \
	done
	@test -f $(NETSTACK_GUEST)/component.wasm \
	  || { echo "missing $(NETSTACK_GUEST)/component.wasm (make wasm-modules)"; exit 1; }
# and no committed artifact may carry a builder-local absolute path. guest-builder
# remaps the checkout, CARGO_HOME and RUSTUP_HOME prefixes to stable tokens (see
# `remap_flags`), so a `/home/...` or `/Users/...` in the bytes means an artifact
# built without that remap — the state where every box disagrees on every module.
	@leaks=$$(git ls-files -z '*.wasm' | xargs -0 grep -laE '/home/|/Users/' || true); \
	  test -z "$$leaks" || { \
	    echo "builder host paths embedded in: $$leaks"; \
	    echo "rebuild with make wasm-modules (guest-builder --remap-path-prefix)"; exit 1; }
	@echo "wasm module artifacts are mutually consistent"

## the reproducibility gate: one guest built from TWO different checkout paths
## must be byte-identical, and carry no host path. Needs the wasm32 target and
## wasm-tools (which `wasm-modules-check` deliberately does not), so it stands
## apart from the pre-push `test` gate. See ops/wasm-repro-check.sh.
wasm-repro-check:
	@bash ops/wasm-repro-check.sh

clean:
	$(CARGO) clean
