# ducktape build + install entry points.
#
# `make node` / `make coordinator` build the runnable product surfaces (the
# networked node daemon and the untrusted UDP coordinator). `make install`
# installs the `ducktape` operator CLI into ~/.cargo/bin. `make test` is the
# full local verification gate — run it before every push.

CARGO ?= cargo
BIN_DEST ?= $(HOME)/.cargo/bin

.PHONY: all demo-seed demo-app demo-clear dogfood-forge node coordinator coordinator-smoke install install-node install-coordinator test clean wasm-modules wasm-modules-check labs-gate

## build every workspace crate (the default target)
all:
	$(CARGO) build --workspace

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

## install the ducktape operator CLI into ~/.cargo/bin
install: install-node

install-node:
	$(CARGO) install --path bin/node --locked

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
test: wasm-modules-check
	$(CARGO) test --workspace
# the #[ignore]d tests are ignored ONLY because they must not share a process
# with the parallel suite — they still have to run. `absolute_configs_resolve_
# after_launch_cwd_is_deleted` re-execs the test binary, and doing that under 32
# live libtest threads made unrelated tests fail ~4 runs in 11 with integrity
# errors. Serial + its own invocation is the isolation. See #887.
	$(CARGO) test -p node-bin --bin ducktape -- --ignored --test-threads=1
	$(CARGO) test -p consensus --features sim
# ducktape-app rides this line for a reason: `cargo test` builds the TEST
# target, which links dev-dependencies, so a product path reaching for a
# dev-only crate compiles under every test lane and breaks only the BINARY.
# That is not hypothetical — it shipped and sat on dev for 81 commits.
	$(CARGO) build -p noded -p simnode -p ducktape-app

## rebuild every wasm guest module into its componentized artifact and refresh
## EVERY committed copy in one sweep (the canonical node-embedded artifact +
## the kernel test fixtures), so the copies can never drift apart. requires
## the wasm32-unknown-unknown target (rustup target add wasm32-unknown-unknown)
## and wasm-tools (cargo install wasm-tools). component bytes are toolchain-
## dependent: a rebuild on a different rustc may legitimately differ from the
## committed bytes — commit the refreshed set TOGETHER; `wasm-modules-check`
## guards mutual consistency, not reproducibility.
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
  crates/modules/system/tagging crates/modules/system/dispatch \
  crates/modules/system/capability crates/modules/system/identity \
  crates/modules/system/gateway crates/modules/system/governance \
  crates/modules/system/saga

# Modules that additionally ship an INDEX guest (src/index_guest.rs behind the
# `index-guest` feature): guest-builder --index writes the canonical
# index.wasm (core wasm, no componentize) into the module directory, which
# noded embeds via include_bytes!. The reference testmap mapper is the
# indexer crate's test fixture and rides the same sweep.
INDEX_MODULES := \
  crates/modules/apps/chat crates/modules/apps/tasks crates/modules/apps/pages \
  crates/modules/apps/inbox crates/modules/system/saga \
  crates/kernel/index-guest/testmap

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
## module must be byte-identical (bin/node embeds the canonical artifact; the
## kernel test fixtures pin the same bytes). toolchain-independent, so it rides
## the pre-push `test` gate; run `make wasm-modules` to refresh the set.
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
	@echo "wasm module artifacts are mutually consistent"

clean:
	$(CARGO) clean
