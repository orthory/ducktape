# Wave 2 integration QA — the one terminal pass

- **Date:** 2026-07-26
- **Status:** runbook, not yet executed. **Nothing in it has been run.**
- **Turns into an executable procedure:** the "Integration QA — one terminal
  pass" section of `2026-07-25-service-daemons.md`.
- **Predecessors:** `2026-07-25-services-extraction.md` (wave 1),
  `2026-07-25-service-daemons.md` (wave 2).
- **Target tree:** `origin/dev` **plus the six still-open PRs** (§0.2). Not dev
  as it stands.

The campaign's own record is why this document exists: **live QA has caught
dead-on-arrival bugs three separate times while every unit gate was green.**
So every step below states an observable and a way to tell a pass from a
skip. A step that cannot fail has been cut.

---

## 0. Before anything

### 0.1 The two rules this runbook never breaks

1. **Never `pkill -f`.** It has already killed an agent's own shell in this
   repo. Every teardown here identifies a process by **cwd + `/proc/<pid>/exe`
   + its `--config`/`--root` argument**, or asks the node to stop itself.
2. **Wait on events, never on durations.** Every wait below names a log line,
   a committed height, a file, or a ws frame. Where a poll loop is used it
   polls **for a state transition**, not for a clock.

Helper used throughout (paste once per shell):

```bash
# wait until a line appears in a log file. Fails loudly, never silently.
await_line() {  # await_line <file> <grep-pattern> [max-polls]
  local f="$1" pat="$2" max="${3:-600}" n=0
  while [ $n -lt "$max" ]; do
    [ -f "$f" ] && grep -qE -- "$pat" "$f" && { echo "SAW: $pat"; return 0; }
    n=$((n+1)); read -r -t 1 </dev/zero 2>/dev/null || sleep 1
  done
  echo "NEVER SAW: $pat  (in $f)" >&2; return 1
}

# THE readiness gate for a node. NOT "does /v1/status return 200".
# `ducktape node run` binds its HTTP listener BEFORE the actor's first status
# publish, and `NodeStatus.public_key` is a plain non-Option String, so in that
# window /v1/status answers 200 with public_key "". #821 closed this for the
# embedded `ducktape-noded` only — the real node's window is still open.
await_published() {  # await_published <http base>
  local base="$1" n=0
  while [ $n -lt 600 ]; do
    local pk; pk=$(curl -sf "$base/v1/status" 2>/dev/null \
      | python3 -c 'import sys,json;print(json.load(sys.stdin).get("public_key",""))' 2>/dev/null)
    [ -n "$pk" ] && { echo "PUBLISHED: ${pk:0:8}"; return 0; }
    n=$((n+1)); sleep 1
  done
  echo "NODE NEVER PUBLISHED an identity at $base" >&2; return 1
}

# the ONLY sanctioned way to stop a ducktape process in this runbook.
stop_by_config() {  # stop_by_config <substring of the process' --config/--root arg>
  local needle="$1" hit=0
  for p in $(pgrep -u "$USER" -x ducktape 2>/dev/null); do
    readlink "/proc/$p/exe" 2>/dev/null | grep -q 'ducktape$' || continue
    tr '\0' ' ' < "/proc/$p/cmdline" 2>/dev/null | grep -q -- "$needle" || continue
    echo "SIGTERM $p ($(tr '\0' ' ' < /proc/$p/cmdline | head -c 120))"
    kill -TERM "$p"; hit=1
  done
  [ $hit -eq 1 ] || { echo "no matching ducktape process for $needle" >&2; return 1; }
}
```

### 0.2 PR dependency map — read this before a partial merge

Six PRs are open; **#821 merged into `dev` at 15:28 on 2026-07-26** and is now
baseline, not a dependency. Every step below is tagged with the PRs it needs.
If a PR has not landed, its steps must be **marked SKIPPED in the report**, not
silently passed.

| PR | branch | what a step needs it for |
|---|---|---|
| **#818** | `feat/airlock-plug` | the `airlock` service kind exists at all; the refusal taxonomy; port-scoped routes |
| **#819** | `fix/announce-all-service-kinds` | a kind is announced on chain; `announce_tag_illegal`; 1-then-every-32nd rejection log |
| **#820** | `fix/service-build-gate` | build skew warns instead of refusing; `deny_unknown_fields` on the agent wire |
| **#822** | `fix/keyless-daemon-config` | the daemon reads `public_key` from `/v1/status` and holds no private key |
| **#823** | `fix/admin-operator-gate` | `admin.token`; `/v1/admin/*` refusal reasons |
| **#826** | `test/claim-lane-e2e` | libpod pulls on 404 — **without it, no container runs on a fresh node at all** |
| ~~#821~~ | *merged* | baseline — but see the correction below |

> **#821 does NOT do what it is easy to assume.** Its readiness fix
> (`booted` `sync_channel`, bind-after-first-publish) is in
> **`bin/noded/src/main.rs`** — the embedded/test daemon `ducktape-noded` — and
> in `bin/noded/src/testkit.rs`. **`bin/node` (the real `ducktape node run`) is
> untouched.** The real node still binds its HTTP surface at
> `bin/node/src/main.rs:382` and does not publish until `:471`. Everything in
> between answers `/v1/status` with **200** and `public_key: ""`.
> **Consequence: use `await_published`, never a 200 check.** This is the single
> most likely way for this pass to start on a false premise.

**#826 is the hard gate.** Without it every Tier 1 and Tier 2 step fails at the
first container create with an image-not-known 404. If #826 has not landed,
**stop** — the pass is not runnable.

**#819 and #818 must be tested together.** #818 gives the airlock daemon an
*empty* capability list, so on #818 alone the lender is invisible in the
committed registry. #819's `kinds.insert` (`bin/node/src/validator/announce.rs:274`)
is what makes `{"providers":{"capability":"airlock"}}` return it. Testing #818
alone yields a green airlock that no peer can discover.

### 0.3 The integration tree does not build itself — four merge conflicts

Verified today with `git merge-tree` against `origin/dev` @ `3754edbda`:

| pair | conflicting file |
|---|---|
| `fix/admin-operator-gate` **vs dev** | `bin/noded/tests/daemon_e2e.rs` (#821 rewrote it under them) |
| `feat/airlock-plug` × `fix/service-build-gate` | `bin/node/src/services.rs` |
| `feat/airlock-plug` × `fix/keyless-daemon-config` | `bin/node/src/services.rs`, `bin/node/src/cred_cli.rs` |
| `fix/keyless-daemon-config` × `fix/service-build-gate` | `bin/node/src/services.rs` |

`bin/node/src/services.rs` is a three-way conflict. **P-2 below is not
`git merge` seven times; it is a real integration branch with hand-resolved
conflicts.** Budget for it, and make the resolution its own reviewable commit —
a botched resolution here would be indistinguishable from a wave-2 defect for
the rest of the pass.

### 0.4 What this pass does NOT prove — assert none of this

State these in the report so a green run is not over-read:

- **`grant.scopes` is not enforced anywhere.** It is minted
  (`bin/node/src/services.rs:895-899`), declared (`scopes_for`, `:1050`) and
  rendered (`:521`, `:567`) — and **read to gate something: nowhere**. Only two
  consumers of a grant exist and neither looks at `scopes`
  (`validator/announce.rs:76` reads `capabilities`; `config/resolve.rs:177`
  reads presence only). **Do not write an assertion implying a scope gates
  anything.** Enforcement is `2026-07-26-wave3-scope-enforcement.md`, unbuilt.
- **`/v1` is unauthenticated** apart from `origin_guard`, which only refuses a
  request that *carries* a non-allowlisted `Origin` — every non-browser client
  passes untouched. A "the daemon could not do X over `/v1`" claim is false by
  construction.
- **`service-link.token` names nobody.** It is one bit — "this process can read
  the node's workspace" — verified `bool`, never "which grant".
- **After #822 a daemon holds no node private key.** That is a real narrowing
  and worth asserting (V-5). It is *not* the same as the daemon being
  authorized, because there is nothing to be authorized against yet.
- **Tart has no egress firewall.** The nft path is podman-only. A macmini Tart
  run can still reach the host's tailnet. Deferred by decision, not a finding.
- **The lent-credential last mile for interactive `claude` on macOS is a known
  open gap** — macOS claude reads OAuth from the login keychain and its TUI
  gate wants full account metadata the lending model deliberately withholds.
  T2-6 is scoped around this; see its note.

---

## 1. Preconditions (P) — 9 steps

### P-1 — the two environment traps, closed explicitly
**Needs:** nothing. **Run on:** both boxes.

`pasta` and `crun` live off the default `PATH`; `nft` is in `/sbin`. A bare
`which pasta` says "missing" and it is not.

`SandboxBackend::probe()` (`crates/services/sandbox/src/sandbox.rs:84-101`)
requires **podman + pasta + nft + nsenter**. `find_on_path` searches `PATH`
only; `find_system_tool` (`podman_api.rs:334-347`) additionally searches
`/usr/sbin`, `/sbin`, `/usr/bin`, `/bin` — so `nft` and `nsenter` resolve
without help, and **`pasta` does not**.

```bash
# dev box — put this in EVERY shell that runs a node or a daemon
export PATH="$HOME/.local/opt/podman-debian13/root/usr/bin:$PATH"
```

**Observable / pass:** all four resolve, and each prints a real path:
```bash
for t in podman pasta crun; do printf '%-8s %s\n' "$t" "$(command -v "$t" || echo MISSING)"; done
printf '%-8s %s\n' nft "$(command -v nft || ls /sbin/nft)"
printf '%-8s %s\n' nsenter "$(command -v nsenter)"
```
**Fail:** any `MISSING`. **This must not be treated as a skip** — see P-9.

### P-2 — build the integration tree
**Needs:** all six PRs. **Run on:** both boxes.

```bash
cd /home/eddy/dev/ducktape
git worktree add .worktree/wave2-qa -b qa/wave2-integration origin/dev
cd .worktree/wave2-qa
for b in fix/admin-operator-gate feat/airlock-plug fix/service-build-gate \
         fix/keyless-daemon-config fix/announce-all-service-kinds test/claim-lane-e2e; do
  git merge --no-edit "origin/$b" || { echo "RESOLVE CONFLICTS in $b, then: git merge --continue"; break; }
done
cargo build --release -p node-bin --bin ducktape
```

Worktree location is mandated by `CLAUDE.md` (`<primary-checkout>/.worktree/<slug>`,
never `/tmp` — it may be memory-backed).

**Observable / pass:** a `release/ducktape` binary exists AND identifies itself:
```bash
./target/release/ducktape --version
git -C . rev-parse HEAD          # record this; it is the integration SHA
git status --porcelain | head    # MUST be empty — a dirty tree changes the build stamp
```
**Fail:** unresolved conflicts, or a non-empty `git status` (a dirty tree makes
`DUCKTAPE_BUILD` a working-tree digest, which will read as skew in R-3 for the
wrong reason).

**macmini:** same commit, native ARM build. It has no `cargo` by default —
rustup was installed there previously. Binary at
`~/dev/ducktape/target/release/ducktape`.

### P-3 — confirm both boxes are on the same build
**Needs:** #820. **Run on:** both.

`DUCKTAPE_BUILD` is stamped at compile time by `bin/noded/build.rs` from the
commit plus a working-tree digest when dirty. It is `option_env!`, so **setting
it at runtime does nothing**.

**Observable / pass:** both boxes print the same integration SHA from P-2, and
`ducktape service status` later shows `build` with no `(this node: …)` suffix
(§R-3). **Fail:** different SHAs — fix before proceeding; every later skew
assertion becomes meaningless.

### P-4 — reap the box's leaked podman services **before** starting anything
**Needs:** nothing. **Run on:** dev box.

**Finding, current as of writing:** this box carries **102 orphaned
`podman system service` processes**, all parented to `init`, all created today
by the PR test suites, across 32 `/tmp/.tmpXXXX` roots, with 96 stale sockets
under `/run/user/1000/ducktape/`. They were launched `--time=0`, so they never
idle-timeout. They will confuse C-1/C-2 (co-tenancy) and can exhaust the socket
directory.

```bash
ls /run/user/1000/ducktape/ | wc -l
pgrep -u "$USER" -f -c 'podman.*system service'     # count only — do NOT kill by pattern
```

Reap by verified identity, never by pattern:
```bash
for p in $(pgrep -u "$USER" -x podman); do
  args=$(tr '\0' ' ' < "/proc/$p/cmdline")
  case "$args" in
    *"system service"*"/tmp/."*|*"system service"*"/tmp/pmtest"*)
      echo "reaping $p: ${args:0:120}"; kill -TERM "$p" ;;
  esac
done
# then sweep the sockets whose service is gone
for s in /run/user/1000/ducktape/*.sock; do
  [ -S "$s" ] || continue
  pgrep -u "$USER" -x podman >/dev/null && \
    pgrep -u "$USER" -a -x podman | grep -qF -- "$s" || rm -f "$s"
done
```

**Observable / pass:** `pgrep -c` returns 0 and the socket dir is empty.
**Fail:** a survivor whose cmdline points at a **workspace** (not `/tmp`) — that
is a live node's service; investigate, do not kill.

### P-5 — workspace layout, and the node config
**Needs:** nothing. **Run on:** both.

Registry root is `$DUCKTAPE_HOME/workspaces` when set, else
`~/.ducktape/workspaces` (`bin/node/src/config/mod.rs:714-721`). Use
`DUCKTAPE_HOME` to keep the pass off the user's real workspaces:

```bash
export DUCKTAPE_HOME="$HOME/.ducktape-wave2qa"
```

Layout after init — every path is real, verified against a live workspace:

```
$DUCKTAPE_HOME/workspaces/<CHAIN-ID>/
  node.toml                 # the config
  network.toml              # chain_id, validators, reach
  identity.key              # node ed25519 secret, 0600
  services.toml             # THE GRANT FILE — created by `service enable`, deleted when empty
  service-link.token        # 32B hex, 0600, minted EVERY boot
  admin.token               # 64 hex chars, 0600, minted EVERY boot         [#823]
  gateway-routes.json       # port-scoped local routes                       [#818]
  daemon.log                # the NODE's stderr tee (NOT the daemons' — see P-7)
  storage/
    services/compute/podman/{storage,run,hooks,owner.pid,podman.pid}
    services/agent/podman/{storage,run,hooks,owner.pid,podman.pid}
    airlock-creds/{<name>/,seal.key}
    agent-workspaces/  agent-sessions/  term-sessions/  forge-repo/
  agent-runs/<16-hex salt>/     # SIBLING of storage, not inside it
```

Podman sockets are **not** under the data dir (108-byte `sockaddr_un` cap):
```
${XDG_RUNTIME_DIR}/ducktape/ducktape-<fnv1a32 of the data dir, 8 hex>-compute.sock
${XDG_RUNTIME_DIR}/ducktape/ducktape-<same tag>-agent.sock
```
The tag is a hash of the **data dir path**, so it changes if the workspace
moves. Graph root and runroot are keyed by **kind**; the **instance id appears
in neither** — it appears only in the container label (C-1).

### P-6 — the `[sandbox]` table (the second trap)
**Needs:** nothing. **Run on:** both.

`NodeToml.sandbox` is `Option<SandboxToml>` with `deny_unknown_fields`. The
**retired flat spelling** (`sandbox = "podman"`) is a serde *type* error, not a
bespoke message: the process dies with
`FATAL: "<path>/node.toml": … invalid type: string "podman", expected struct SandboxToml`.
**Assert on the substring `FATAL:` plus `SandboxToml`**, not on a full sentence —
the span rendering is `toml` 0.8's and is not pinned by any test.

The header must be **last** in `node.toml` (everything after a TOML table header
belongs to it). All four keys required; `0` = probe the host.

Dev box (Linux/podman):
```toml
[sandbox]
runtime = "podman"
image = "docker.io/library/node:22-slim"
cores = 0
mem_gb = 0
```
macmini (Tart):
```toml
[sandbox]
runtime = "tart"
image = "ghcr.io/cirruslabs/macos-sonoma-base:latest"
cores = 2
mem_gb = 4
```

**Observable / pass:** the node boots and, with no compute grant yet, logs
exactly:
```
WARN ducktape::service: sandbox configured but the compute service is not enabled; this node will run no provider work and announce no capabilities — enable it with `ducktape service run compute` … reason="compute_not_granted"
```
That warn is the proof the table parsed. **Fail:** `FATAL: … SandboxToml`.

### P-7 — daemon logs are NOT in `daemon.log`
**Needs:** nothing.

`ducktape service run <kind>` calls `noded::log::init(None, None)`
(`bin/node/src/services.rs:933`) — **stderr only, no file, no log ring.** Only
the *node* tees into `<workspace>/daemon.log`. Every daemon in this runbook is
started with its stderr captured explicitly:

```bash
RUST_LOG=info,ducktape::service=debug,ducktape::saga=debug \
  ./target/release/ducktape service run compute -n "$CHAIN" \
  > "$LOGS/compute.out" 2> "$LOGS/compute.log" &
```
`ducktape::saga=debug` is not optional: `lease_renew_failed` and
`result_submit_failed` are `debug`, and K-2 needs them.

### P-8 — `admin.token`, and the trap the prompt does not mention
**Needs:** #823.

The credential is 32 random bytes rendered as **64 lowercase hex chars**,
written `create_new` at **0600**, **freshly minted every boot** (the old file is
removed first). Path is `<dir>/admin.token` where `<dir>` is:
- **`ducktape node run`** → the workspace dir beside `node.toml` ✅ (our case)
- **embedded `ducktape-noded`** → `<storage>/admin.token` (different! not used here)

```bash
ADMIN="$(cat "$WS/admin.token")"
curl -s -o /dev/null -w '%{http_code}\n' -XGET "http://127.0.0.1:$HTTP/v1/admin/ping" \
  -H "x-ducktape-admin-token: $ADMIN"
```

Gated routes, the full list (`bin/noded/src/admin.rs:620-640`):
`GET /v1/admin/ping`, `POST /v1/admin/shutdown`, `GET /v1/admin/logs/tail`,
`POST /v1/admin/module-code/stage`, `GET /v1/admin/module-code/{digest}`.

> **`/v1/log-filter` is NOT gated.** It is on the public router
> (`bin/noded/src/lib.rs:550`). `CLAUDE.md:106`'s
> `curl -XPOST localhost:$PORT/v1/log-filter -d '…'` keeps working with no
> header. Do not "fix" it.

> **THE TRAP — the token is not always sufficient.** `admit_gate`
> (`bin/noded/src/admin.rs:348-377`) first resolves the node's owner from the
> `identity` module. If the node **has** a committed owner account — which it
> does the moment you run `user account-init`, i.e. from T2-1 onward — the
> operator-token path is **not taken at all**; the request needs an **owner
> PoP**: `x-ducktape-admin-key`, `x-ducktape-admin-ts`, `x-ducktape-admin-sig`.
> Only a node with `NoOwner` accepts the bare token.
>
> Mint the PoP with the CLI (stdin: the key password):
> ```bash
> ducktape user sign-admin --key "$WS/user.key" \
>   --method POST --path /v1/admin/shutdown --node-key "$NODE_HEX"
> # -> {"key":"<hex>","ts":"<secs>","sig":"<hex>"}
> ```
> then send those three as the `x-ducktape-admin-{key,ts,sig}` headers. `ts` must
> be the one that was printed — it is inside the signed bytes, along with the
> **target node's** key (no cross-node replay).
>
> **Consequence for `ops/demo-clear.sh`:** #823 patched it to send the token, but
> on an *owned* node that request now gets `403 not_the_owner`; the script falls
> through to its pid sweep, so it still works — but do not read its success as
> proof the admin gate accepted anything. Record this as a real (small) gap.

**Never paste the token or its file contents into a report.**

### P-9 — the anti-silent-skip discipline
**Needs:** nothing. **This is the step the whole runbook hangs on.**

The campaign was already burned by a suite that "passed" because every daemon
exited at boot. Two live instances of that hazard exist **today**:

1. **`bin/node/tests/dispatch_e2e.rs:491-494` and `:654-657` skip with a bare
   `return`.** The test reports **`ok`/PASSED**, not `ignored`. The `eprintln!`
   is captured by libtest and **discarded on success**. On a host where
   `probe()` refuses — which is exactly a host that forgot P-1 — you see two
   green tests that ran nothing. If you run them at all:
   ```bash
   cargo test -p node-bin --test dispatch_e2e -- --nocapture 2>&1 | tee dispatch.out
   grep -c 'skipping ' dispatch.out        # MUST be 0
   grep -c 'compute daemon serving' "$LOGS"/*.log   # MUST be >= 1
   ```
2. **`dogfood_loop_e2e`** has the identical missing-`[sandbox]` defect and is
   dead on `dev`; **`sched_pinned_run`** gates on the weaker `podman version`
   predicate and so *fails* rather than skips on such a host. Neither is part of
   this pass; note them as pre-existing.
3. **Liveness before probe, always.** #821's `await_status`
   (`bin/noded/tests/daemon_e2e.rs:75-92`) now checks `child.try_wait()` **before**
   probing the port, because a node that lost a port race exits — and a
   probe-first loop then adopts *a stranger's* 200 as its own readiness and
   drives someone else's node for the rest of the run. Its panic says it
   outright: *"If something still answers that port, it is NOT ours."* Mirror
   that here: before every `await_published`, confirm the pid you started is
   still alive.

**A fourth hazard, silent by construction.** A malformed `Create` frame is
dropped by the agent daemon with the serde message **discarded** (`Err(_) =>` at
`bin/node/src/agent/link.rs:180`), one WARN `reason="malformed_command"`, and the
ws link stays attached and healthy-looking — while the node's
`TermSessions::start` awaits the reply with **no timeout at all**
(`bin/noded/src/term.rs:896-900`, deliberate: "a cold image pull legitimately
takes minutes"). So the symptom is *a session create that never returns and
never errors*. Any step that creates a session must treat "no answer" as a
FAIL with that grep, never as slowness.

**Rule for every step below:** a step reports **PASS**, **FAIL**, or
**SKIPPED(reason, PR)** — never blank. A tool that is absent is a **FAIL of
P-1**, never a skip of the step that needed it.

---

## 2. Tier 1 — no airlock at runtime (T) — 8 steps

The claim under test: **airlock is a credential SOURCE, not a dependency.**
broker-host is always in the path (per-run loopback + opaque bearer); the
operator's own credential resolves locally and never touches an airlock.

Topology for this tier: **dev box alone.** One node, both daemons.

### T1-1 — found the network, node up
**Needs:** baseline.

```bash
export DUCKTAPE_HOME="$HOME/.ducktape-wave2qa"
export PATH="$HOME/.local/opt/podman-debian13/root/usr/bin:$PATH"
D=/home/eddy/dev/ducktape/.worktree/wave2-qa/target/release/ducktape

"$D" node init --name w2qa \
  --listen 127.0.0.1:59300 --advertised 127.0.0.1:59300 \
  --http 127.0.0.1:9971 --rpc 127.0.0.1:9974 \
  --wireguard-listen 0.0.0.0:59320 --invite-listen 0.0.0.0:59321 \
  --primary-coordinator none
CHAIN=$("$D" node list | awk '/w2qa#/{print $1; exit}')
WS="$DUCKTAPE_HOME/workspaces/$CHAIN"
# then edit $WS/node.toml per P-6 ([sandbox] table LAST)
LOGS="$HOME/wave2qa-logs"; mkdir -p "$LOGS"
RUST_LOG=info "$D" node run -n "$CHAIN" > "$LOGS/node.out" 2>&1 &
```

**Wait on:** `await_published "http://127.0.0.1:9971"` — **not** a 200 check, for
the reason in §0.2. `app surface listening` in `daemon.log` marks the *bind*,
which is strictly earlier than the first publish and must not be used as the
gate.

**Observable / pass, all four:**
- `daemon.log` carries `INFO ducktape::node: node boot node=<hex8> version=… binary=… built_unix=…`
- `daemon.log` carries `INFO ducktape::consensus: … genesis root_hash=<64 hex>` — **record this hash; it is the V-1 anchor**
- `daemon.log` carries the `reason="compute_not_granted"` warn from P-6
- `test -s "$WS/admin.token" && test -s "$WS/service-link.token"` — both exist, both `0600` **[#823]**
- after `await_published`, `/v1/status` carries a non-empty `public_key`, a real
  `version`, and a real `root_hash`:
  ```bash
  curl -s "http://127.0.0.1:9971/v1/status" | head -c 400
  ```

**Record, do not fail on:** the size of the pre-publish window. On `ducktape node
run` it spans `main.rs:382` (bind) to `:471` (first publish) and answers 200 with
`public_key: ""` throughout. Measured on `ducktape-noded` before #821 it was
~1.3 s. **Worth timing here and reporting** — it is the window T1-2's exit lands
in, and closing it for `bin/node` is the obvious follow-up PR.

**Fail:** no `admin.token` — #823 did not land; mark every P-8/V-4 assertion SKIPPED.

### T1-2 — compute signals, and is refused nothing
**Needs:** #822 (keyless boot), #820 (build is metadata).

```bash
RUST_LOG=info,ducktape::service=debug,ducktape::saga=debug \
  "$D" service run compute -n "$CHAIN" --no-enable \
  > "$LOGS/compute.out" 2> "$LOGS/compute.log" &
```
`--no-enable` is deliberate: it proves the non-TTY path emits **one line** and
keeps serving, which is what a systemd unit does.

**Observable / pass:**
- `compute.log` (stderr) carries the banner, **with no ANSI escapes** because
  stderr is a file:
  `● compute · signaling to <CHAIN> · offering <tags>`
- then exactly one hint line: `not enabled — enable it with: ducktape service enable compute`
- **no prompt, no spinner, no re-ask** — grep the file for `[Y/n]`: must be 0 hits.
- the catalog sees it:
  ```bash
  curl -s http://127.0.0.1:9971/v1/services | python3 -m json.tool
  # -> {"signaling":[{"kind":"compute","version":"0.1.0","capabilities":[...],"scopes":[...],"needs":[]}]}
  ```
- `"$D" service list -n "$CHAIN"` renders `· compute  signaling  -`

**Fail modes worth naming:**
- process exits with `sandbox: <detail>` → P-1 was not done (probe is
  fail-closed **before** the first hello).
- **[#822] the shortened fuse.** `node_identity()` is now the *first* hard
  dependency on a live node — earlier than `send_hello` used to be, by one whole
  `backend.probe()` (which on a cold host takes seconds). There are **zero
  retries**: the first failure propagates and `bin/node/src/main.rs:126-141`
  prints `FATAL: <err>` to **stderr** (not tracing) and `exit(1)`.
  Two distinct lines, and **the second is misleading — flag it as a finding**:

  | when | exact stderr line |
  |---|---|
  | node not listening | `FATAL: could not read this node's identity: the node is not running` |
  | node listening, not yet published | `FATAL: this node published a 0-byte mesh identity, not 32 — build mismatch` |

  The second says **"build mismatch"** and will send an operator hunting a
  version skew that does not exist. The message the code *intends* for this case
  (`this node has not published a mesh identity yet — start it, then start the
  daemon`) is **effectively unreachable**: `NodeStatus.public_key` is a plain
  non-`Option` `String`, so it is always present as `""`, and `config::unhex("")`
  returns `Ok(vec![])` — so the empty case falls through to the 0-byte arm.
  **Assert both:** that the daemon exits 1 and does NOT spin (correct), and that
  the pre-publish message is wrong (a finding to file).
- HTTP 409 `build_mismatch` on the hello → #820 did **not** land. Under #820
  nothing is refused for build; see R-3.

### T1-3 — the consent boundary, non-interactively
**Needs:** baseline.

```bash
INSTANCE=$("$D" service enable compute -n "$CHAIN" -y)    # stdout is the id ALONE
echo "$INSTANCE"                                          # -> compute#xxxxxxxx
```

**Observable / pass:**
- stdout is exactly `compute#<8 hex>` and nothing else (so `$(...)` is scriptable)
- stderr carried the red-painted consent summary before it:
  `service / node / status / offers / grant scopes` — with `status` = green `signaling`
- `$WS/services.toml` now exists, `version = 1`, one `[[service]]` with
  `kind = "compute"`, `instance` = 64 lowercase hex, `nonce` = 32 lowercase hex
- the *daemon's own* stderr — not the CLI's — records
  `INFO ducktape::service: … "service enabled" instance=compute#…` only if the
  daemon minted it; here the CLI did, so this line is absent by design
- `"$D" service status -n "$CHAIN" --json | python3 -m json.tool` gives, **with
  #820**, **9** keys: `kind, state, instance, version, build, capabilities,
  scopes, needs, unmet_needs`, with `state: "enabled"`. Without #820 there are
  8 (no `build`) — a good cheap check of which tree you are on.
  Note `service list --json` emits the same shape but the **table** shows only
  `KIND / STATE / INSTANCE`; `build` is a `status`-only row label.

**[#822] `service enable` now needs a live node.** It used to read
`identity.key` for the consent screen's node id; it now takes the same
`node_identity()` → `/v1/status` path (`services.rs:1274`). So enable requires
**both** a published node **and** a live hello already in the catalog. If it
fails here, check `await_published` before suspecting the grant path.

**Assert the negative:** `"$D" service enable broker -n "$CHAIN" -y` must fail
with `broker is not signaling to this node, so there is nothing to consent to
— start it first: ducktape service run broker`. **broker and sandbox are
libraries, never enable-able services** — this is the plan's own litmus.

> Note: `<KIND>` is **not** a closed enum. Any `1..32 chars of [a-z0-9-]` is
> accepted, signals, and executes nothing (`daemon_for` returns `None` →
> `Served::SignalOnly` → the process parks). So the assertion above proves
> "there is no broker daemon", not "the CLI rejects the word broker".

### T1-4 — the compute daemon actually serves
**Needs:** #822.

Restart the daemon so it picks up its grant (it read the grant once, at boot):
```bash
stop_by_config "service run compute"     # or fg + ^C
RUST_LOG=info,ducktape::service=debug,ducktape::saga=debug \
  "$D" service run compute -n "$CHAIN" > "$LOGS/compute.out" 2> "$LOGS/compute.log" &
```

**Wait on:** `await_line "$LOGS/compute.log" 'compute daemon serving'`
**Observable / pass:** the line carries `instance=compute#<hex8>`,
`capabilities=<n>`, `concurrency=4` (or `$DUCKTAPE_MAX_CONCURRENT_RUNS`).
Its own podman service is up and answers:
```bash
ls "$WS/storage/services/compute/podman/"        # storage run hooks owner.pid podman.pid
SOCK=$(ls "$XDG_RUNTIME_DIR/ducktape/"*-compute.sock)
curl -s --unix-socket "$SOCK" http://d/_ping     # -> OK
```
**Fail:** `another service daemon (pid <N>) already owns <socket> — stop it
before starting this one` → a previous daemon survived; use `stop_by_config`,
never a pattern kill.
**Fail:** `podman service did not answer on <path> within 5s`.

### T1-5 — agent signals and serves alongside compute
**Needs:** #822, #820.

```bash
RUST_LOG=info,ducktape::service=debug \
  "$D" service run agent -n "$CHAIN" --enable \
  > "$LOGS/agent.out" 2> "$LOGS/agent.log" &
```
`--enable` exercises the assume-yes path (mint without asking).

**Wait on:** `await_line "$LOGS/agent.log" 'agent daemon serving'`
**Observable / pass:**
- `agent daemon serving` carries `instance=agent#<hex8>` and `cap=<MAX_TERM_SESSIONS>`
- `"$D" service list -n "$CHAIN"` shows **two** `✓` rows with **different**
  instance ids and different kinds
- the agent daemon claimed the ws link — it sent
  `{"op":"service_attach","kind":"agent","token":"<service-link.token>"}`.
  **[#820] the claim no longer carries `build`**; the node compares no stamp.
  Confirm no `link_refused` in `agent.log`.
- two podman services now, two graph roots:
  ```bash
  ls "$WS/storage/services/"                     # agent  compute
  ls "$XDG_RUNTIME_DIR/ducktape/"*-agent.sock "$XDG_RUNTIME_DIR/ducktape/"*-compute.sock
  ```

**Fail:** `reason="link_refused"` with
`refused: present this node's service-link token, and only one agent service may
attach` → the token was stale (the node restarted) or a second agent is running.

### T1-6 — an operator-owned credential, no airlock in the path
**Needs:** baseline.

```bash
"$D" user account-init --name eddy -n "$CHAIN"     # password on stdin
"$D" user cred add claude -n "$CHAIN"              # browser OAuth
```
Headless alternative, if a real login is unavailable:
`DUCKTAPE_CRED_REUSE_ARTIFACT=<path to a real ~/.claude/.credentials.json>`
imports an existing login instead of driving the browser.

**Observable / pass:**
- `$WS/storage/airlock-creds/<name>/` exists with a `kind` marker and the vendor
  artifact; `$WS/storage/airlock-creds/seal.key` exists at **0600**
- `cred add` **auto-published** the on-chain `airlock` RouteStatement (#767
  behaviour) — no hand-built JSON
- **[#818]** `cred add` now prints `lend it by running: ducktape service run airlock`
- **[#818]** the node's boot warn `reason="airlock_not_granted"` (target
  `ducktape::service`, carries `credentials=<count>`) appears on the **next node
  restart** — the store is now non-empty and no airlock grant exists. It is a
  warn, not a refusal; the node keeps serving.

> **From here the node has a committed owner.** Every later `/v1/admin/*` call
> needs the owner PoP, not the token (P-8).

### T1-7 — headless run, end to end, airlock-free
**Needs:** #826 (the pull), #819 (so the run can be placed by tag).

```bash
RUN=$("$D" agent sched claude --cred eddy-claude-1 -n "$CHAIN" --cpu 1 --mem 2 -- "reply with exactly: PONG")
echo "$RUN"        # -> sched<0x1F><32 hex>   NOTE: literal ASCII unit separator
DISPATCH=${RUN#*$'\x1f'}
```

**Wait on — the events, in order:**
1. `await_line "$LOGS/compute.log" 'compute daemon serving'` (already true)
2. the run appears committed:
   ```bash
   curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
     -d '{"target":"runs","query":"pending_runs"}'
   ```
   poll **for the transition** out of `pending_runs` and into `recent_runs`
3. tail the live output ring (there is **no CLI verb** — this is the ws topic):
   subscribe to `ws://127.0.0.1:9971/v1/ws` with
   `{"op":"subscribe","topics":["run-output:'"$DISPATCH"'"]}`

**Observable / pass:**
- `recent_runs` carries this run with a success outcome and the model's `PONG`
- a real container ran: `grep -c 'io.ducktape.managed=compute#' ` in the podman
  socket's container list during the run (see C-1 for the exact query)
- **the airlock daemon was never started and nothing 502'd**: `grep -cE
  'airlock_gateway_unreachable|airlock_route_or_credential_absent|gateway_seal_pk_mismatch'
  "$LOGS"/*.log` → **0**. *This is the tier's whole point.*
- `"$LOGS/compute.log"` has **no** `reason="worker_error"`

**Fail:** a 404-shaped container-create failure → **#826 did not land**.
**Fail:** any `airlock_*` reason token → airlock is on the runtime path, which
contradicts the design claim. Report it as the headline finding.

### T1-8 — interactive pty session, airlock-free
**Needs:** #820 (the `Create` wire), #822.

```bash
"$D" agent pty claude -n "$CHAIN" --cpu 1 --mem 2
```

**Observable / pass:**
- stderr prints `attached to <session_id> (term:<session_id>)`
- a real TUI renders; typing echoes; a terminal resize propagates (SIGWINCH →
  `{"op":"term_resize",…}`)
- the session ends cleanly on provider exit — the node emits a
  `{"type":"term_ended",…}` frame and the CLI shuts down its background reader
  (this is the #779/#780 wedge fix; a hang here is a regression)
- `agent.log` shows the `TermCreate` decode succeeded

**[#820] — the deliberate-failure half of this step.** `Create` is now
`deny_unknown_fields` with `limits` and `credential` **required**. Prove the
gate actually refuses, by hand-driving one malformed frame at the agent
daemon's ws link:

The minimal valid frame (from the PR's own test) is:
```json
{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{},"credential":null}
```

| frame mutation | must be refused because |
|---|---|
| omit `"limits"` | `missing field \`limits\`` — previously `#[serde(default)]` → empty map = "provider defaults" |
| omit `"credential"` | `missing field \`credential\`` — an `Option` normally decodes absent → `None` = "the operator's own credential". `#[serde(deserialize_with = "Option::deserialize")]` (`wire.rs:83`) is what suppresses that fallback. **This was the widest fail-open: silence granted authority.** |
| add `"spend_cap": 5` | `unknown field \`spend_cap\`` |
| add an unknown key to a `ClientMsg` | `BadFrame` naming the field |

> **The observable is NOT an error message — read this before running it.**
> `Create` flows node → daemon, and the daemon's `classify`
> (`bin/node/src/agent/link.rs:178-188`) matches `Err(_)` and **discards the
> serde text**. What you actually get:
> - one WARN, target `ducktape::service`, `reason="malformed_command"`, message
>   `agent daemon dropped a frame it could not decode` — **no field name, no
>   session, no detail**
> - the ws link stays **attached and healthy** (`Incoming::Ignore`, read loop
>   continues)
> - the node's `TermSessions::start` **hangs forever** — there is deliberately
>   no timeout (`bin/noded/src/term.rs:896-900`), because a cold image pull
>   legitimately takes minutes. It is released only when the daemon detaches.
>
> **So: PASS = the create never completes AND exactly one
> `reason="malformed_command"` appears. FAIL = the session is created.** Do not
> wait for an error; there will not be one.

**The reverse direction behaves differently, and mislabels itself.** A daemon
`Event` carrying an unknown field fails at the node, which replies
`ServerFrame::Error { code: BadFrame, detail: <serde msg> }`. The daemon's
`classify` matches `type: "error"` unconditionally → logs **ERROR**
`reason="link_refused"` with the serde text as `detail`, and **closes the
connection**. So a per-frame decode error is reported to the operator as a *link
refusal*. Assert the detail text names the offending field — that is what makes
it diagnosable at all.

---

## 3. Tier 2 — credential lending through the airlock daemon (T2) — 6 steps

Cross-box. **Lender = dev box** (holds the credential, runs `service run
airlock`). **Borrower = macmini** (runs compute/agent, executes the run).

> Reuse of prior topology: `dukenet#03f6df3d` from the 2-node campaign is
> **gone from this box** (the workspace no longer exists). The tailnet is live —
> `zk` `100.76.154.57`, `macmini-duke` `100.110.104.117` — and LAN between them
> is dead (same NAT, client isolation, hairpin blocked), so **tailnet only**,
> `primary_coordinator = "none"`. See §6 for what I could not determine.

### T2-1 — two nodes, one chain, over the tailnet
**Needs:** baseline.

Both `node.toml`s: `advertised = "<tailnet-ip>:59300"`,
`wireguard_advertised = "<tailnet-ip>:59320"`, `primary_coordinator = "none"`.

**Wait on:** each node's `daemon.log` reaching the same finalized height.
**Observable / pass:** `"$D" node status -n "$CHAIN"` on both boxes reports the
**same height and the same root hash**; `node peers` shows the WireGuard
handshake `COMPLETE`.
**Known flake:** the overlay tunnel has been observed flapping (peer DARK every
~3 min, then re-handshake). Sync stays on tip, but a credential handshake landing
in a DARK window can fail intermittently — if T2-5 fails once, re-run it and say
so rather than recording a defect.

### T2-2 — the airlock plug on the LENDER
**Needs:** **#818**.

```bash
# dev box (lender)
RUST_LOG=info,ducktape::gateway=debug \
  "$D" service run airlock -n "$CHAIN" --enable \
  > "$LOGS/airlock.out" 2> "$LOGS/airlock.log" &
```

**Wait on:** `await_line "$LOGS/airlock.log" 'airlock daemon serving'`
**Observable / pass, in this exact order** (`bin/node/src/airlock.rs:101-171`):
1. store opens (a broken store fails the **process**)
2. binds loopback:0 — the port is always ephemeral, there is **no `--port`**
3. `gateway-routes.json` gains `{"name":{"label":"airlock"},"port":<N>}`
4. `airlock daemon serving`
5. `reason="airlock_store_empty"` **only if** the store is empty — with T1-6 done
   it must be **absent**
6. the heartbeat spawns

```bash
"$D" gateway list -n "$CHAIN"     # -> [{"name":{"label":"airlock"},"port":<N>}]
ss -ltnp "sport = :<N>"           # the daemon is actually listening
```

**Assert the plug offers no capability tags.** `service status` shows
`offers  -` for airlock and `scopes  gateway.credentials, gateway.routes`. This
is correct: a lending laptop has no container runtime, and #818 runs **no
sandbox probe** for airlock.

**Assert the heartbeat.** Hand-unbind the route and watch it come back:
```bash
"$D" gateway unbind --label airlock -n "$CHAIN"
# poll `gateway list` until the route RETURNS — reassert re-registers a Vacant slot
```
The beat is `HELLO_TTL/3` = **10 s**, logged on beat 1 then every **30th**
(`LOG_EVERY = 30` — note: **not** 32; 32 is #819's two unrelated constants).

### T2-3 — the kind is discoverable on chain
**Needs:** **#818 + #819 together.**

```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":{"providers":{"capability":"airlock"}}}'
```
**Observable / pass:** the lender's node key is in `providers`.
**Fail:** empty → this is precisely the #818-alone hole: an airlock grant with
zero executor tags is announced **only** because #819 inserts the kind itself
(`announce.rs:274`, test `a_kind_with_no_executors_still_announces_itself`).

Also check the borrower announces its kinds:
```bash
curl -s http://<macmini>:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":"all"}'
```
**Pass:** the borrower's row contains `compute` and `agent` as tags in their own
right, alongside executor tags. **There is no CLI verb for this** — `service
list`/`status` read the *local* catalog, never the committed registry.

### T2-4 — grant the credential to the borrower's account
**Needs:** baseline.

```bash
# macmini
"$D" user account-init --name duke -n "$CHAIN"
# dev box
"$D" user cred grant eddy-claude-1 <macmini-account-hex> -n "$CHAIN"
```
**Observable / pass:** the `gateway` module's credential record lists the
grantee. **Do not assert that any scope is enforced** (§0.4).

### T2-5 — a lent-credential run, cross-box
**Needs:** #818, #819, #826.

```bash
# from the dev box, pinned to the macmini
"$D" agent sched claude --cred eddy-claude-1 --node duke -n "$CHAIN" -- "reply with exactly: PONG"
```

**Observable / pass:**
- the run executes **on the macmini** and returns `PONG`
- the borrower's broker opened a sealed airlock session: **zero** refusal
  tokens in the borrower's logs
- the lender's `airlock.log` shows the grant gate *deciding*, at `debug`, target
  `ducktape::gateway`, carrying `credential=<name>` and **never** the account

**This is the step most likely to catch something.** The failure surface is the
whole point of #818's taxonomy; §5 maps each token to its cause.

### T2-6 — interactive pty on a lent credential
**Needs:** #818.

```bash
"$D" agent pty claude --cred eddy-claude-1 --node duke -n "$CHAIN"
```

**Expected outcome is qualified, and that is deliberate.** The console rendering
cross-box was proven in the earlier campaign. The **lent-credential last mile
for interactive `claude` on macOS is a known open gap**: macOS claude reads OAuth
from the login keychain, and its TUI auth gate checks full account metadata
(`accountUuid`, `billingType`, …) that the lending model withholds by design.

- **PASS** = the console renders and the session is interactive.
- **KNOWN-GAP** = the console renders and claude shows "Select login method".
  That is the documented gap, not a wave-2 regression. Record it as KNOWN-GAP.
- **FAIL** = anything that does not reach the provider at all — a refusal token,
  a spawn failure, a wedge on child exit.

If the macmini Tart backend is the blocker, the honest fallback is a
Linux/podman borrower, where the headless token-only path is already proven.

---

## 4. On/off isolation matrix (I) — 6 steps

**The claim:** separate processes mean separate failure domains. With all three
enabled, toggle each independently and prove the others are unaffected.

### I-0 — state what `disable` does and does not revoke
**Needs:** baseline. **This is an assertion about truth, not a wish.**

`disable` (`bin/node/src/services.rs:1203-1230`) removes the grant from
`services.toml`, prints the retired id, and **that is all**:

| `ducktape service disable compute` | |
|---|---|
| removes the `[[service]]` row from `services.toml` | ✅ immediately |
| retires the instance id (a re-enable mints a **fresh** one) | ✅ |
| the node retracts the announce | ✅ **on the next announce tick** — the announcer re-reads `services.toml` every tick, no restart needed |
| stops the daemon process | ❌ never |
| tears the ws link down | ❌ never |
| kills running containers | ❌ never |
| cancels in-flight work | ❌ never — the daemon read its grant once, at its own boot |

The CLI says so itself:
`stop the daemon too: a running `service run compute` keeps executing work it
already holds`. **Assert that sentence, not a revocation.**

### I-1 — disable compute while an agent pty is live
Start a pty session (T1-8). With it live, `service disable compute`.
**Pass:** the pty session **keeps running**, uninterrupted; `service list` shows
compute gone from the grants and agent still `✓ enabled`.
**Fail:** the pty drops.

### I-2 — disable agent while a compute run is in flight
Submit a long `agent sched` run, then `service disable agent`.
**Pass:** the run completes and delivers its result.

### I-3 — disable airlock while a lent-credential run is in flight
**Needs:** #818.
**Pass:** the in-flight session survives (`SESSION_TTL_SECS = 3600`,
`MAX_REQUESTS = 4096` are per-session and fixed); a **new** session started
after the daemon stops gets `airlock_gateway_unreachable`.
**Also assert:** after a clean SIGTERM the `airlock` entry is **gone** from
`gateway-routes.json` and, if it was the only route, the file is **removed**, not
left as `{"routes":[]}`.

### I-4 — kill a daemon uncleanly; prove the blast radius is one daemon
Send **SIGKILL** to the compute daemon (identified via `stop_by_config`'s
`/proc/<pid>/exe` check, then `kill -KILL`).
**Pass:** the node stays up; the agent daemon stays serving; the compute row
transitions `enabled` → `enabled-but-absent` in `service list` once its catalog
entry lapses (`HELLO_TTL = 30 s`) — **poll for the state string, do not sleep
30 s and look once**.
**Known residual to assert honestly [#818]:** a SIGKILLed *airlock* daemon
leaves its route in `gateway-routes.json` forever — nothing re-validates the
port and there is no eviction. The borrower is not misled (connect-refused →
502 → `airlock_gateway_unreachable`), so what is missing is the eviction, not
the diagnosis. This is the PR's own documented gap; record it, do not file it as
new.

### I-5 — a deliberately crashing plug
Run a bogus kind that signals and executes nothing:
`"$D" service run notaservice -n "$CHAIN" --no-enable`, then SIGKILL it.
**Pass:** it appears as `· notaservice signaling`, then lapses out of the
catalog; nothing else moves. This also documents that `<KIND>` is an open
string, not an enum.

---

## 5. Podman co-tenancy (C) — 3 steps

**The claim:** each daemon reaps only its own id-labelled containers, and one
daemon's shutdown cannot kill the other's.

### C-1 — separate roots, separate sockets, scoped labels
**Needs:** baseline. With compute and agent both serving and both busy:

```bash
CSOCK=$(ls "$XDG_RUNTIME_DIR/ducktape/"*-compute.sock)
ASOCK=$(ls "$XDG_RUNTIME_DIR/ducktape/"*-agent.sock)
for s in "$CSOCK" "$ASOCK"; do
  echo "== $s"
  curl -s --unix-socket "$s" 'http://d/v5.0.0/libpod/containers/json?all=true' \
    | python3 -c 'import sys,json; [print(c["Labels"].get("io.ducktape.managed"), c["Labels"].get("io.ducktape.node")) for c in json.load(sys.stdin)]'
done
```

**Observable / pass, all four:**
- the label key is `io.ducktape.managed` and the value is the **display id**:
  `compute#<hex8>` on one socket, `agent#<hex8>` on the other
- a second label `io.ducktape.node=<execution node id>` is present
- **neither socket can even enumerate the other's containers** — that is the
  real mechanism (per-service graph roots + runroots), and the disjoint labels
  are defence in depth
- graph roots are distinct dirs: `storage/services/compute/podman/storage` vs
  `.../agent/podman/storage`

**Fail:** any container labelled `io.ducktape.managed=unscoped` — that is a
provider built outside `discover(…, managed_owner)` and nothing will ever reap
it.

### C-2 — one daemon's shutdown cannot kill the other's containers
Stop the compute daemon (SIGTERM via `stop_by_config`), while an agent session
holds a live container.
**Pass:** the agent's container is still `running` on the agent socket after the
compute daemon and its podman service are gone.
**Why it holds:** `PodmanService` supervises its child with `kill_on_drop`; a
*shared* service would die with whichever daemon started it and take the other's
containers along. That is exactly why per-service services were chosen over
labels.

### C-3 — crash-orphan re-adoption across restart
SIGKILL the compute daemon mid-run so a container is left behind, then restart it.
**Wait on:** `await_line "$LOGS/compute.log" 'reaped orphaned sandbox containers'`
**Pass:** the line carries `removed=<n>` and `reason="own_orphans"`; the agent's
containers are untouched. This is the second reason instance ids must survive a
restart — a daemon re-adopts its own containers only if it returns with the
**same** id, which it does because the id is the grant hash and the grant
persists in `services.toml`.
**Fail:** `reason="reap_failed"`, or the count includes the agent's containers.

---

## 6. Cold start (K) — 2 steps

### K-1 — a genuinely cold node takes a run
**Needs:** **#826.**

Make the image store genuinely empty (do this **with the daemon stopped** —
`PodmanService::claim` treats two supervisors on one root as the hazard it
exists to prevent):
```bash
stop_by_config "service run compute"
rm -rf "$WS/storage/services/compute/podman/storage"
```
Restart the daemon, then submit one run (T1-7).

**Expected behaviour, stated up front:**
1. `create` POSTs `/containers/create`, gets **404 image not known**
2. it calls `POST /images/pull?reference=<image>` and retries `create`
3. the run proceeds normally

**Observable / pass:**
- the run completes, and the image is now present in **that daemon's** store
  (`curl --unix-socket "$CSOCK" http://d/v5.0.0/libpod/images/json`)
- the *agent* daemon's store is **still empty** — one image store per service is
  the accepted cost, and this proves it
- **no** `worker_error` in `compute.log`

**Fail:** the run fails at create → #826 is not in the tree.

> **The pull is completely invisible.** `crates/services/sandbox/src/podman_api.rs`
> contains **zero `tracing::` calls**, so there is no "pull started"/"pull
> finished" line and no duration. A cold first run looks exactly like a hang.
> Budget several minutes for `node:22-slim` (~230-250 MiB) and say so in the
> report; do not treat the silence as a wedge.

> **A trap in the pull path itself:** libpod's pull endpoint returns **HTTP 200
> on a *failed* pull** — the verdict is an `{"error":…}` line inside the
> streamed body (`pull_failure`, `podman_api.rs:924-930`). If a run fails right
> after a cold start with no obvious cause, that is where to look.

### K-2 — the claimed residual: does a cold winner lose its lease to its own download?
**Needs:** #826.

The PR's stated residual is that the pull happens at first run *inside* the
lease window, so a cold winner can lose its lease to its own image download.

**The arithmetic does not support that story, and this step exists to settle
it.** An agent run's lease is `RUN_LEASE_VIEWS = 1024` views
(`crates/modules/apps/runs/src/lib.rs:172`) at `BLOCK_TIME = 1 s` ≈ **17
minutes**; the host heartbeat fires every **10 s** (`compute/pool.rs:41-47`) and
is `select`ed against the run future, so it covers the create/pull; and each
renewal past the half-window resets expiry to `height + 1024`. No path was found
that makes a `node:22-slim` pull outlast that.

**So the expected result is: the cold run completes and the lease is never
lost.**

**How to tell a pass from a failure:**
- **PASS:** run completes; `grep -c 'lease_renew_failed' "$LOGS/compute.log"` = 0;
  the saga shows **one** attempt.
- **RESIDUAL CONFIRMED (report loudly):** `recent_runs` shows the run on
  `attempt: 1` or higher, **or** `lease_renew_failed` appears. Then capture
  `RUST_LOG=ducktape::saga=debug` output — the cause is either the `RenewLease`
  origin check refusing (`saga/src/lib.rs:930-936`) or the renew submit failing.
- **NOTE:** on lease expiry the attempt is **cancelled and re-placed**, not
  dropped: `lease_and_request` recomputes the assignee by rendezvous over
  `(saga_id, attempt, height)`. `RUN_MAX_ATTEMPTS = 2`, so a *second* expiry
  fails the saga with `lease attempts exhausted`. Nothing is silently lost.
- **There is no "lease lost" or "run re-placed" log line at all** — `saga` is a
  consensus module and emits no `tracing`. The only observable is committed
  state via `SagaQuery`/`runs` queries.

---

## 7. Restart and skew (R) — 4 steps

### R-1 — daemon restart keeps the instance id
Stop and restart the compute daemon.
**Pass:** `service status` shows the **same** `compute#<hex8>`; `services.toml`
is unchanged; the daemon re-adopts its own containers (C-3).
**Then** `service disable compute` + `service enable compute` and assert the id
is **different** — a re-enable mints a fresh nonce and therefore a fresh id.
That asymmetry is the consent-epoch property; both halves must hold.

### R-2 — node restart, daemons still up
**Needs:** #822, #823.

Restart the node with the daemons left running.
**Observable / pass:**
- `admin.token` and `service-link.token` are **both freshly minted** (contents
  differ from before — compare hashes, never print them)
- the **agent daemon's ws link is refused** until it re-reads the new
  `service-link.token`, then reconnects. Expected transient in `agent.log`:
  `reason="link_refused"` … `refused: present this node's service-link token…`,
  followed by a successful redial. **A permanent wedge here is a FAIL.**
- the compute daemon rides through: its heartbeat logs
  `reason="hello_failed" attempts=1` (then every 30th) while the node is down and
  `"signal restored"` after
- **[#822]** a daemon started *while the node is down* exits loudly with
  `could not read this node's identity: …` and does **not** spin. Assert the
  process is gone, not looping.

### R-3 — deliberate build skew must WARN, not refuse
**Needs:** **#820.**

Build a second binary at a different commit (or with one file touched) and run
the compute daemon from **that** binary against the node from P-2's binary.

**Observable / pass:**
```
WARN ducktape::service: … kind=compute reason="build_skew"
  "this daemon and its node are different builds; restart the daemon from the node's build if they disagree about the protocol"
```
and **the daemon keeps serving** — a run submitted afterwards still completes.
`service status` renders the `build` field as
`<daemon build> (this node: <node build>)` in yellow.

Matched builds instead log `INFO … "daemon and node builds agree"`. A build that
cannot be identified on either side is `Skew::Unknown` and **warns about
nothing** — an unknown build is not evidence of skew.

**What is compared:** the daemon's own `build_identity_or_unknown()` against the
`build` field the node returns in the **200 body of `POST /v1/services/hello`**
(`{"ttl_secs":30,"build":"…"}`) — so the diagnostic is a round trip, not a local
guess.

**"Latched" means once per transition, scoped to one `service run` process.** The
latch is a local `Option<Skew>` inside `heartbeat()`, seeded from the startup
hello. So a skewed daemon warns exactly **once at boot** and stays silent for the
rest of its life unless the node's stamp changes under it. Beat is 10 s; without
the latch that would be 6 warns a minute. **Assert the count is 1, not ≥1.**

**A cheap negative worth taking:** the CLI's `render_build` does a raw string
compare and has no `Unknown` state, while the log has three. So an `unknown`
daemon against a real-stamped node **renders as skew in `service status` but
logs nothing**. Note the disagreement rather than treating either as the bug.

**Two more #820 behaviour changes to confirm while you are here:**
- `service run` **no longer refuses to start on a git-absent build**. It used to
  exit `this binary has no build identity; rebuild it from a git checkout`; it
  now signals `build = "unknown"` and serves. Test it by building from a tarball
  or with `.git` hidden.
- the `enabled-but-absent` hint no longer mentions builds. It is now
  `enabled but not signaling — is its daemon running (ducktape service run), and
  pointed at this node's http surface?` — **if you still see the old text naming
  `reason build_mismatch`, #820 is not in the tree.**

**Fail:** HTTP **409 `build_mismatch`** on the hello (`this node runs build <id>;
restart the service daemon from the same build`) → **#820 did not land**. Mark
R-3 SKIPPED(#820), not FAILED.

> **The one skew that is still hard, and it is NOT the build stamp.** #820
> deleted `ServiceAttach.build` **and** put `deny_unknown_fields` on `ClientMsg`.
> So a **pre-#820** daemon (which still sends `"build"`) attaching to a
> **post-#820** node is refused with a `BadFrame` naming the field. That is
> correct and intended — but it means "skew warns instead of refusing" is true
> *within* post-#820 builds and false *across* the #820 boundary. Say which you
> tested.

### R-4 — restart inside vs beyond the lease window
Kill the compute daemon mid-run and restart it promptly.
**Pass:** the daemon re-adopts its container and the saga stays on **attempt 0**
(the lease is renewed from the restored heartbeat).
Then repeat, leaving it down long enough for the lease to lapse: the attempt is
cancelled and re-placed (attempt increments). With `RUN_MAX_ATTEMPTS = 2`, a
second lapse fails the saga with `lease attempts exhausted`. Assert via
`runs`/`saga` queries — there is no log line.

---

## 8. Cross-node placement (X) — 3 steps

### X-1 — agent on A, compute only on B
**Needs:** #819, #826.

Disable compute on the dev box; leave it enabled and serving on the macmini.
Submit an unpinned run from the dev box.
**Pass:** the run executes on the macmini and its result commits. Proof the
placement was real, not local:
```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":{"capable_providers":{"capability":"compute","demands":{"cores":1}}}}'
```
returns only the macmini's key, and the macmini's `compute.log` — not the dev
box's — carries the run.

### X-2 — the kind tag is in the committed registry
**Needs:** **#819.**
```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":"all"}'
```
**Pass:** each node's announced set is **sorted** and contains its granted kinds
(`compute`, `agent`, `airlock`) *as tags*, plus the executor-tag intersection of
`grant.capabilities ∩ hello.capabilities` **per kind**. A daemon's hello can
never vouch for another kind's tag.
**Fail:** kinds absent → #819 not in the tree.

**Deliberate-failure half.** Make a daemon offer an illegal tag (a capability
spec whose tag has an uppercase letter or a space — the hello boundary accepts
any printable ASCII, the registry accepts only `[a-z0-9._-]`, ≤64 bytes).
**Pass:**
```
WARN ducktape::service: … reason="announce_tag_illegal" dropped=<n>
  "a signaling daemon offers capability tags the registry refuses …"
```
the **legal** tags still announce, and the warn is **latched** (once per
transition, not per tick), with a clearing `INFO … "every offered capability tag
is well-formed again"`.

### X-3 — a rejected announce reports, then re-arms
**Needs:** #819.

Force announce rejections (e.g. submit while the node cannot apply).
**Pass:** the warn appears at attempts **1, 32, 64, …** — never on every tick:
```
WARN ducktape::modules: … reason="capability_announce_rejected" attempts=<n> … "capability announce did not apply; retrying"
```
(validator form carries `height` + `detail`; the resident form carries `outcome`
and no `height`.)

> **You cannot directly observe the 32-block re-arm.** `on_blocks` →
> `submit_failed` has **zero `tracing` calls**. The only signal is that a later
> tick emits a fresh `"capability announce submitted"` at **debug** — so run
> `ducktape::modules=debug` or you will see nothing. Also: `ANNOUNCE_RETRY_BLOCKS
> = 32` and `REJECTION_REPORT_EVERY = 32` are two different constants that share
> the value 32; a "32" in a log identifies neither.

---

## 9. Invariants to assert at the end (V) — 5 steps

### V-1 — consensus root hash unchanged by any of it
Record the genesis `root_hash` from T1-1 and compare at the end:
```bash
grep 'genesis root_hash=' "$WS/daemon.log" | tail -1
"$D" node status -n "$CHAIN"
```
**Pass:** the genesis root is byte-identical on both boxes and unchanged from
T1-1. Service enable/disable/restart touches **no consensus module** — the whole
structural claim of the campaign.
Cross-check the workspace-independent gate:
```bash
make wasm-modules-check
```
**Fail:** any difference → a service change reached consensus, which it must not.

### V-2 — `/v1` additive only
```bash
git diff origin/dev...HEAD -- bin/noded/src/lib.rs | grep -E '^\-.*\.route\('
```
**Pass:** zero removed routes. New routes are fine; a changed or removed one is
a wire break.

### V-3 — no credential name or token in any log
```bash
grep -rniE 'admin\.token|service-link|sk-|Bearer |accessToken|refreshToken' \
     "$WS/daemon.log" "$LOGS"/*.log | grep -v 'admin.token in the node' | head
```
**Pass:** no secret **values**. Note that a credential **name** legitimately
appears (`credential=<name>` on the airlock grant gate) — that is by design; the
**account** never does, and neither does any token.
Also assert the doctrine directly: **no URI path or query string is logged** —
`/.duck/ws/{token}` carries a capability in the path.
```bash
grep -nE 'path=|uri=|/\.duck/ws/' "$WS/daemon.log" "$LOGS"/*.log | head   # expect empty
```

### V-4 — the admin gate actually refuses
**Needs:** #823. Run against a node **before** T1-6 (no owner yet) so the
operator-token arm is the one under test:

| request | expected status | expected `reason` |
|---|---|---|
| no header | **401** | `operator_token_missing` |
| wrong header value | **403** | `operator_token_mismatch` |
| correct header | **200** | — |
| from off-box (tailnet IP) | **403** | `admin_off_box` |
| with `DUCKTAPE_ADMIN=off` | **404** | `admin_namespace_absent` |

Body shape is `{"error":…,"reason":…}`. **Node-side the refusal is logged at
`debug`**, target `ducktape::admin` — set `RUST_LOG=ducktape::admin=debug` or you
will see nothing.
On an **owned** node (post-T1-6) the same requests yield `not_the_owner` /
`owner_signature_invalid` instead; assert that too, and that
`ducktape user sign-admin` produces headers the node accepts.

### V-5 — no daemon holds the node private key
**Needs:** **#822.**
```bash
for p in $(pgrep -u "$USER" -x ducktape); do
  tr '\0' ' ' < "/proc/$p/cmdline" | grep -q 'service run' || continue
  # a daemon must never have identity.key open
  ls -l "/proc/$p/fd" 2>/dev/null | grep -c 'identity.key'
done
```
**Pass:** `0` for every daemon process, and the **node** process is the one that
answers for the identity. Structural proof: `ServiceConfig` has no field a secret
could live in, `resolve_service` never opens `identity.key`, and containment runs
one way (`Resolved` holds a `ServiceConfig`, never the reverse).
**Also assert the honest caveat** from the code's own doc: `/v1/status` carries
no auth, so a same-uid process that binds `http_listen` first could answer with
any `public_key`. That grants a same-uid attacker nothing they could not get by
reading `identity.key` directly — **not a regression, not a blocker**, and it
becomes load-bearing only when a daemon has no workspace.

---

## 10. Failure triage table

Every expected failure mode, and the one string that identifies it. All reason
tokens are stable snake_case and greppable.

### Borrower side — broker (`crates/services/broker/src/lib.rs`, WARN, target `ducktape::gateway`, message `airlock session not opened: …`)

| `reason` | what actually happened |
|---|---|
| `airlock_gateway_unreachable` | transport error with no HTTP status, **or** HTTP 502. The lender's daemon is down, or its node cannot reach the daemon's loopback port. **Includes the stale-route case** (route registered, nothing listening). |
| `airlock_route_or_credential_absent` | HTTP 404. No `airlock` route published on the lender, or no credential by that name in its store. |
| `credential_not_granted` | HTTP 403. The lender's grant gate refused this account. **Also fires when the request carried no/undecodable `account_b64` at all** — a 403 where no grant was ever consulted. |
| `airlock_grant_authority_unavailable` | HTTP 503. The lender's gate could **not ask** its authority (10 s query timeout, node restarting, resident not yet serving). Never means "denied". |
| `airlock_gateway_refused` | any other HTTP status. |
| `airlock_gateway_malformed_response` | body not JSON / wrong shape / truncated / non-base64 token. |
| `gateway_seal_pk_mismatch` | well-formed response whose sealed token will not open under the pinned key — the gateway's real seal key ≠ the published one. **Historically this masked every pinned-path failure**; the taxonomy above is what fixed that, so seeing it now means what it says. |
| `airlock_session_unclassified` | **a code defect, not an ops condition.** Unreachable by construction; it exists so a new failure path that forgets to tag itself does not borrow another arm's name. |

### Lender side — the grant gate (`bin/node/src/airlock.rs`, DEBUG, target `ducktape::gateway`, carries `credential=<name>`, never the account)

| `reason` | trigger | answer |
|---|---|---|
| `credential_record_absent` | query OK, gateway module returned no record for that name | 403 |
| `grant_authority_unavailable` | the `/v1/query` itself failed — **note: no `airlock_` prefix on this side** | 503 |
| `credential_not_granted` | record exists; the account is neither owner nor grantee | 403 |

### Lender side — operational

| `reason` | level | meaning |
|---|---|---|
| `airlock_store_empty` | WARN | store opened with 0 credentials. Not fatal. |
| `airlock_not_granted` | WARN (node, target `ducktape::service`) | credentials present, no airlock grant. Boot-time only. |
| `airlock_cred_incomplete` | WARN | a credential dir missing its `kind` marker or artifact. Skipped, not fatal. |
| `route_owned_by_another_daemon` | WARN (heartbeat) / INFO (shutdown) | the port is a `Foreign` owner; refuses to steal or delete it. |
| `route_refresh_failed` / `route_retire_failed` | WARN | `gateway-routes.json` unwritable or non-canonical. |
| `signal_handler_install_failed` | WARN | the route will **not** be retired on exit. |

### Service lifecycle

| symptom | identifier |
|---|---|
| daemon exits before signaling | `sandbox: <detail>` → **P-1 not done** |
| daemon exits at boot, node down | `could not read this node's identity: …` **[#822]** |
| daemon exits at boot, node up but unpublished | `this node has not published a mesh identity yet — start it, then start the daemon` **[#822]** |
| hello refused | HTTP 400 `malformed_hello` / 503 `catalog_full`. **#820 deleted both build refusals** — a 409 `build_mismatch` or a 503 `build_identity_unavailable` means #820 is not in the tree. |
| daemon and node on different builds | WARN `reason="build_skew"` once per transition — **serves anyway** **[#820]** |
| agent ws link refused, token/holder | `reason="link_refused"`, detail `refused: present this node's service-link token, and only one agent service may attach` |
| **mixed binaries: a `dev`-built daemon against a #820 node** | ERROR `reason="link_refused"` whose `detail` is a serde message `unknown field \`build\`, expected \`kind\` or \`token\`` — because the old `service_attach` still sends `build` and `ClientMsg` now denies unknown fields. **Silent on the node side** (no counter, no reason tag beyond the frame error) and it reconnects forever. The most likely mixed-binary failure in this pass. |
| a `Create`/`Command` frame the daemon cannot decode | WARN `reason="malformed_command"` — **serde text discarded, link stays up, the node's session create hangs with no timeout** |
| daemon cannot reach the node | WARN `reason="hello_failed" attempts=N` (1, then every 30th); recovery = INFO `"signal restored"` |
| two daemons, one graph root | `another service daemon (pid <N>) already owns <socket> — stop it before starting this one` |
| podman service never came up | `podman service did not answer on <path> within 5s` |
| flat `sandbox =` key | `FATAL: "<path>": … invalid type: string "…", expected struct SandboxToml` |
| `[sandbox]` present, compute ungranted | WARN `reason="compute_not_granted"` |

### Placement / announce

| symptom | identifier |
|---|---|
| illegal capability tag dropped | WARN `reason="announce_tag_illegal" dropped=<n>` (latched) |
| too many tags | WARN `reason="announce_over_cap" dropped=<n> cap=64` (latched) |
| announce rejected in consensus | WARN `reason="capability_announce_rejected" attempts=<1,32,64,…>` |
| grant file unreadable | WARN `reason="grant_unreadable"` |
| run not placed | **no log line** — query `capability` `capable_providers` and `saga` |
| lease renewal dropped | DEBUG `reason="lease_renew_failed"` — invisible at `info` |
| result delivery dropped | WARN `reason="result_lane_closed"` / DEBUG `reason="result_submit_failed"` |
| attempt failed | WARN `reason="worker_error"` |
| projection lost the attempt mid-run | WARN `reason="projection_missing_mid_work"` |

### Admin (`bin/noded/src/admin.rs`, DEBUG, target `ducktape::admin`)

`admin_namespace_absent` (404) · `admin_off_box` (403) ·
`operator_token_unavailable` (503) · `operator_token_missing` (401) ·
`operator_token_mismatch` (403) · `owner_unresolved` (503) ·
`owner_signature_invalid` (401) · `owner_signature_stale` (401) ·
`not_the_owner` (403)

---

## 11. Existing QA recipes: what today's changes touch

Checked `skills/`, `ops/`, `docs/`, `Makefile`, every `.sh`/`.mjs`/`.md`
(excluding `node_modules`/`target`).

- **`skills/qa/SKILL.md` — already updated by #823 itself.** It gains the
  `x-ducktape-admin-token` curl and a "never paste that token" line. **No
  separate edit is needed.** One thing it still does not say: on a node with a
  committed owner the token is refused and an **owner PoP** is required (P-8).
  Worth a follow-up line, in #823 or after it.
- **`ops/demo-clear.sh` — already updated by #823**, and gated on the token
  being readable so it falls through to its pid sweep otherwise. See P-8 for why
  its admin call will 403 on an owned node.
- **`ops/worktree-clean.sh` — untouched and unaffected.** It has no `curl` and
  no `/v1/` anywhere; it reaps by pidfile + `/proc/<pid>/exe` + `--config`
  identity, and never uses `pkill -f`.
- **`ops/agent-system` — unaffected.** Only `/v1/query` and `/v1/submit`. It
  remains the fastest way to read `runs pending_runs` / `recent_runs` and
  `capability all`.
- **`ops/demo-seed.sh`, `demo-app.sh`, `demo-gateway.mjs`, `demo-kanban.mjs`,
  `dogfood-forge.sh` — unaffected.**
- **`CLAUDE.md:106`'s `/v1/log-filter` recipe — still correct.** That route is
  not gated.
- **`Makefile:33`** references a stale `/v1/shutdown` in a comment (the route
  moved to `/v1/admin/shutdown` long ago). Cosmetic, pre-existing.
- **`docs/superpowers/specs/2026-07-14-w2-owner-control-design.md`** still
  describes the loopback-trust admin model. Documentation drift; the client it
  described (`app/admin-client.ts`) no longer exists.
- **`docs/superpowers/plans/2026-07-26-wave3-scope-enforcement.md:143` is now
  wrong.** It documents the service-link authentication as "kind == agent,
  **build equality**, node-wide link token, single holder". #820 deleted the
  build check from `take_service_link` entirely. Wave 3's premise survives, but
  that line should be corrected before anyone builds on it.
- **Nothing in `skills/`, `ops/` or `.project/` references `--compute` or the
  retired flat `sandbox =` key** — verified by grep. The flag day is clean.

**Test-suite hazards this pass must not inherit** (§P-9): `dispatch_e2e` skips
silently as a **PASS**; `dogfood_loop_e2e` has the same missing-`[sandbox]`
defect and is dead on `dev`; `sched_pinned_run` gates on the weaker
`podman version` predicate and fails rather than skips.

---

## 12. Report template

For each step: `PASS` / `FAIL(<reason token or log line>)` /
`SKIPPED(<reason>, <PR>)`. Never blank.

Plus, at the top:
- the integration SHA from P-2 and whether the tree was clean
- **which of the six PRs were actually in the tree**
- the genesis root hash, from T1-1 and from V-1 (must match)
- every step that was SKIPPED, and why
- explicitly: "this pass did **not** test grant-scope enforcement, because none
  exists" (§0.4)

---

## 13. Step count

| section | steps |
|---|---|
| 1. Preconditions (P) | 9 |
| 2. Tier 1 (T1) | 8 |
| 3. Tier 2 (T2) | 6 |
| 4. On/off isolation (I) | 6 |
| 5. Podman co-tenancy (C) | 3 |
| 6. Cold start (K) | 2 |
| 7. Restart and skew (R) | 4 |
| 8. Cross-node placement (X) | 3 |
| 9. Invariants (V) | 5 |
| **total** | **46** |
