# Wave 2 integration QA — the one terminal pass

- **Date:** 2026-07-26. **Revised:** 2026-07-27 against `dev` @ `60d86b8ec`.
- **Status:** runbook, not yet executed. **Nothing in it has been run.**
- **Turns into an executable procedure:** the "Integration QA — one terminal
  pass" section of `2026-07-25-service-daemons.md`.
- **Predecessors:** `2026-07-25-services-extraction.md` (wave 1),
  `2026-07-25-service-daemons.md` (wave 2),
  `2026-07-26-assumption-audit.md`, `2026-07-26-work-admission.md`.
- **Target tree:** `dev` @ **`60d86b8ec`** (PR #839 merged), as it stands. **No
  integration branch, no cherry-picks, no open PRs to wait for.** Code facts
  below were established at `fc6334d8f` and re-checked against `60d86b8ec`; #839
  touched the kernel host only, and `GENESIS_ROOT_HASH` is unmoved.

The campaign's own record is why this document exists: **live QA has caught
dead-on-arrival bugs three separate times while every unit gate was green.**
So every step below states an observable and a way to tell a pass from a
skip. **A step that cannot fail has been cut.**

This is the last preparation before the acceptance test runs. Read it as: *if
we succeed at this, we nailed it.* That standard only means something if every
step below is capable of failing — so where a step's "pass" used to be the
absence of an error, it now names a post-state somebody has to look at.

---

## 0. Before anything

### 0.1 The three rules this runbook never breaks

1. **Never `pkill -f`, and never `until ! pgrep -f …`.** A pattern match has
   already killed an agent's own shell in this repo, and a `pgrep -f` spin loop
   is the same hazard wearing a wait's clothing — it matches the grep, the
   editor, and this script. Every teardown here identifies a process by
   **cwd + `/proc/<pid>/exe` + its `--config`/`--root` argument**, or asks the
   node to stop itself.
2. **Wait on events, never on durations.** Every wait below names a log line,
   a committed height, a file, or a ws frame. Where a poll loop is used it
   polls **for a state transition**, not for a clock.
3. **A pass is an observed post-state, never the absence of an error.** `rm -rf`
   that returns 0 having deleted nothing is the canonical failure of this rule
   (§P-4). If a step's evidence is "no error appeared", it is not evidence.

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
# `ducktape node run` binds its HTTP listener at `boot::surfaces::bind`
# (bin/node/src/main.rs) and does not publish its identity until the status
# cell is filled several hundred lines later. `NodeStatus.public_key`
# (bin/noded/src/lib.rs) is a plain non-Option String, so in that window
# /v1/status answers 200 with public_key "".
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

> **Why `await_published` survives.** #836 fixed exactly this bug — but in
> **`bin/simnode`** ("`/v1/status` must never answer before the actor
> published"), and #821 fixed it earlier in **`bin/noded`**. **`bin/node`, the
> real `ducktape node run`, still binds before it publishes.** Two of the three
> node binaries are closed and the one this pass drives is not. Use
> `await_published`; never a 200 check. This is still the single most likely way
> for this pass to start on a false premise.

### 0.2 What changed under this runbook — read before trusting any step

This runbook was written against `e0d773f68`. **Eighteen PRs merged after it.**
The six PRs it was written to wait for are all in `dev` now, so the old
dependency map, the `**Needs:** #8xx` tags, the four-way merge-conflict
integration branch, and the whole `SKIPPED(reason, PR)` apparatus are **gone**.
Nothing in this pass is skippable for a PR reason any more — which means every
step below must either pass or fail.

| merged | what it invalidated in the version you may remember |
|---|---|
| **#818/#819/#820/#821/#822/#823/#826** | all six "still-open" PRs plus the baseline one. **§0.2's dependency map and §0.3's merge-conflict table are deleted.** `P-2` is `cargo build`, not a hand-resolved integration branch. |
| **#830** | pinned the production genesis root hash as a **test**. "Root hash unchanged" is no longer a manual observation — **V-1**. |
| **#831** | **inverted the skip default tree-wide.** A missing capability now **FAILS**; skipping is opt-in via `DUCKTAPE_ALLOW_MISSING_TOOLS=1`; skip lines go to **fd 2 directly**. Every step that expected a silent skip is now a failure — **P-9**. |
| **#832** | unified CLI node-addressing into **one ladder**. **`agent --node <name>` became `--host-node <name>`**; `agent` gained `--node <http-url>`; `user`/`fs`/`agent` honour `DUCKTAPE_NODE`; trailing slashes trimmed. **Every `--node` in the old runbook meant the wrong thing** — T2-5, T2-6. |
| **#827** | **gated ws topic subscription.** `run-output:` / `term:` / `term-command:` now need the workspace secret in the subscribe frame — **T1-7**. `agent pty` reads that secret before creating a session. |
| **#829** | **`ducktape service enable` is an on-chain transaction** (node-signed, keyless CLI, settle-then-answer, reports a height). `disable` needs a reachable submitting node — **T1-3, I-0**. |
| **#833** | **deleted the caller-asserted account from the credential path.** A grant now names the account of the node that **RUNS** the workload, not the submitter — **T2-4**, §10. |
| **#834** | `duckdns` → `crates/duckdns`, `blobstore` → `crates/kernel/blobstore`; `crates/modules/` is consensus-only (`apps/` + `system/`, 10 each). |
| **#835** | a **shared** pty's command lane is driven by the channel **owner** only; anything else is `command_not_channel_owner`. **No step here exercises it** — see §0.4b. |
| **#836** | fixed `bin/simnode`'s status-during-genesis bug. simnode is **73/73**. |
| **#838** | `[work] admit` — `work-admit.toml`, default owner-only, `ducktape node work {list,admit,revoke}`. **It already amended this runbook**: T2-4b, the X-1a/X-1b split, and four §0.4 bullets. Those amendments are preserved below; build on them. |
| **#839** | deleted the **continuation lane** — the consensus-takeover class. Landed after this revision began; in the tree, **untested by this pass** (§0.4). `GENESIS_ROOT_HASH` unmoved. |

**`bin/node/src/validator/announce.rs` no longer exists.** #829 deleted the
validator announce pump. Any step that cited `announce.rs:274` or
`validator/announce.rs:76` was pointing at a deleted file; those citations are
replaced with the surviving symbols in `bin/node/src/announce.rs`.

**Line numbers have been stripped from most citations on purpose.** They rotted
within a week of the first draft — of the ten load-bearing `file.rs:NNN` refs
spot-checked at `fc6334d8f`, four had drifted and one pointed at a deleted file.
Citations below name a **file plus a symbol**, which is greppable and survives
the next eighteen PRs.

### 0.3 Environment facts — get these wrong and you will chase ghosts

Each of these has already cost this campaign real time. They are not
housekeeping; they are the difference between a real finding and a bogus one.

**`/tmp` is a 63 G tmpfs.** It is *memory*. It was 49 G free after a cleanup and
it refills. A suite that puts its storage there dies with
`No space left on device`, which looks exactly like a real regression — one
agent chased **12 failures that grew to 55** before finding the cause. Right
now this box carries **83 leftover test roots under `/tmp/.tmp*` totalling
4.9 G**, all of them tmpfs, i.e. RAM.

**Mandate a disk-backed `TMPDIR`, outside any worktree:**

```bash
mkdir -p "$HOME/wave2qa-tmp"
export TMPDIR="$HOME/wave2qa-tmp"          # NOT /tmp, NOT inside .worktree/
df -h "$TMPDIR" | tail -1                  # MUST NOT say tmpfs
```

> **Outside any worktree is not a style note.** An agent put a scratch dir
> inside the repo and a later `git add -A` swept **WireGuard keys and admin
> tokens into a commit**. A scratch dir under a checkout is one careless
> `add -A` away from being published. Keep it in `$HOME`.

**`pasta` and `nft` are not on `PATH`, and they are not missing.**

```
pasta   /home/eddy/.local/opt/podman-debian13/root/usr/bin/pasta   (off PATH)
nft     /sbin/nft                                                  (off PATH)
```

`command -v pasta` and `command -v nft` both print nothing on this box.
**Three separate people concluded a tool was absent from a bare `which` and
were wrong.** P-1 is the step that closes this, and it must be run in *every*
shell that starts a node or a daemon.

**`cargo check -p node-bin --tests` does not reach `compute-service`'s test
target.** `compute-service` is a normal dependency of `node-bin`, so `-p
node-bin` compiles its **lib** and never its `#[cfg(test)]` code — a broken test
in `crates/services/compute` passes that gate silently. Use:

```bash
cargo check --workspace --tests
```

**`bin/node/tests/remote_session.rs` is pre-existing red.** It was red before
this pass and is not this pass's to fix. Record it as a known-red baseline; do
not spend the pass on it and do not report it as a wave-2 finding.

### 0.4 What a green pass does NOT prove — state all of this in the report

A green run here proves the **service plane composes and stays out of
consensus**. It proves nothing about authorization. Say so explicitly, because
the shape of this runbook — refusal taxonomies, grant gates, admission steps —
reads like a security pass and is not one.

- **`/v1` is trusted-local, and that is the whole authorization story.** The
  only guard is `origin_guard`, and in the code's own words
  (`bin/noded/src/handle.rs`, on `workspace_secret_matches`): *"`origin_guard`
  passes every `Origin`-less caller and a signaling hello confers nothing."*
  Every non-browser client — curl, the CLI, any process that can reach the port
  — is admitted untouched. **Any claim of the form "the daemon could not do X
  over `/v1`" is false by construction.** Keeping those ports loopback-bound is
  what makes anything below mean anything.
- **`grant.scopes` gates nothing, anywhere.** It is minted, declared
  (`scopes_for`, `bin/node/src/services.rs`), length-validated
  (`bin/noded/src/services.rs`, a wire-size cap, not an authorization check) and
  rendered in `service status`. Grepped tree-wide at `fc6334d8f`: **no consumer
  reads it to gate anything.** Do not write an assertion implying a scope
  authorizes a thing. Enforcement is
  `2026-07-26-wave3-scope-enforcement.md`, unbuilt.
- **A same-uid process can read `identity.key`.** Everything this runbook proves
  about a daemon not holding the node key (V-5) is a **narrowing of blast
  radius within one uid**, not isolation. `/v1/status` carries no auth either,
  so a same-uid process that binds `http_listen` first can answer with any
  `public_key`. That grants an attacker nothing they could not get by reading
  the key file — which is the point: the boundary is the uid, not the process.
- **The `logs` ws topic is Public.** #827 gated three topic families
  (`run-output:<id>`, `term:<session>`, `term-cmd:<session>`) behind the
  workspace secret and deliberately left four open: `module:<id>`,
  `files:watch`, **`logs`**, and `metrics` (`Topic::admission`,
  `bin/noded/src/stream.rs`, pinned by `every_topic_family_has_a_decided_admission`).
  `module:<id>` is the broader one — **every committed op of any indexed module,
  decoded, to any ws caller.** The rationale is in the frame's own doc: *"the
  same bytes already leave this node over an unauthenticated HTTP route."* True,
  and it means #827 narrowed the interactive plane, not the read plane. Note the
  deliberate asymmetry: `logs` is Public while its admin twin
  `GET /v1/admin/logs/tail` is gated.
- **`service-link.token` names nobody.** It is one bit — "this process can read
  the node's workspace" — verified `bool`, never "which grant". It is now doing
  double duty as both the `ServiceAttach` credential and the ws topic
  admission, by design and by the same reasoning ("a second secret would be a
  second thing to leak").
- **Delegation is not implemented, and the refusal you will see is correct.**
  See §0.4a — this is the one most likely to be misread as a defect.
- **The continuation-lane deletion (#839) landed in `dev` at `60d86b8ec`, and
  this pass does not test it.** The consensus-authorship finding
  (`2026-07-26-author-gated-ops.md`) — that `origin` is a *lane*, not an author,
  so a released continuation carried an attacker-chosen `Origin::Module` and any
  key with submit standing reached every `Origin::Module(_)`-gated arm, valset's
  membership gate included — was fixed by **deleting the lane**. Its own repro
  ran on the ordered lane against `fc6334d8f`: `validators before=2 after=3`,
  then `final validator set = 1 key(s)`. **No step below exercises that fix.**
  A green pass here is not evidence the takeover class is closed; the PR's own
  tests are. Do not let the two be conflated in the report.
- **Work admission gates only the CALLER-CHOSEN lanes.** #838 decides whose work
  a node runs for a pinned/announced saga (`agent sched`) and a cross-node pty
  (`agent pty`). A run reaching a node through `RequestRun`, a chat mention, a
  pages comment, forge or the jobs board carries a MODULE saga origin
  (`dispatch`), is not attributable to an account at that layer, and is
  **admitted**. Those lanes cannot name a credential — their payload is composed
  in consensus — so the residual is free compute, not a credential draw. Do not
  assert that admission gates them.
- **Work admission is bounded by `/v1`'s exposure.** `POST /v1/submit` re-signs
  as THIS node, so anything that can reach the node's HTTP or RPC port is
  admitted by construction.
- **Tart has no egress firewall.** The nft path is podman-only. A macmini Tart
  run can still reach the host's tailnet. Deferred by decision, not a finding.

### 0.4a Delegation: A-submits, B-executes, drawing on A's grant — expected to FAIL

**Write this down before anyone runs T2-5, or the correct refusal will be filed
as a bug.**

Since #833 the credential path has no caller-asserted account. The drawing
identity is minted by the node's gateway proxy from the mesh-verified WireGuard
peer (`x-duck-caller-account`, `bin/node/src/gateway_plane.rs`), so **the
gateway sees the node that MADE THE HOP — the executor — and never the
submitter.**

Consequence: **a grant to A does not travel with A's submission.** If A owns the
credential and submits a run that B executes, the lender sees B, B is on no
grant list, and the run is refused `credential_not_granted`. Only a grant naming
**B's own account** works.

This is asserted by a merged test —
`a_delegated_run_draws_as_the_executing_node_not_the_submitter`
(`bin/node/tests/sched_pinned_run.rs`) — which walks all three directions
explicitly:

| direction | setup | asserted outcome |
|---|---|---|
| **0** | executor has not admitted the submitter's account | refused by **work admission**, before the lender is dialled at all. No container, no gateway hop, no session. |
| **1** | admitted; the credential **OWNER** submits; executor ungranted | **still fails** with `credential_not_granted` — "an ungranted EXECUTOR must fail even when the OWNER submitted" |
| **2** | grant the **EXECUTOR's** account | succeeds. That grant is the only thing that changed. |

Delegation — carrying the submitter's authority to the executor — **is
sequenced as its own PR and does not exist here.** The design is written up
(`2026-07-26-work-admission.md` §4, "Delegation — after, not with": a required
tagged `work: WorkRef` on `SessionRequest`, a widened `GrantCheck`, and a
`grant_answer` that verifies `assignee`/`origin` from its own committed saga
state). Its own decision line reads *"Delegation ships next, not with"*, and
justifies the sequencing with the fact this step asserts: *"A's run on B drawing
on **A's** grant has never worked, so sequencing regresses nothing."* **No
implementation exists at `fc6334d8f`.**

In this pass:

- direction 1 failing is a **PASS**, and must be recorded as `EXPECTED-REFUSAL`,
  never `FAIL`;
- T2-4 grants the **borrower's** account, not the submitter's, and the runbook
  says so at the step;
- a tester who "fixes" this by granting the submitter has broken the test, not
  the product.

### 0.4b Two things this runbook deliberately does NOT test

Naming them here stops a later reader adding a step that cannot fail.

- **#835's shared-pty owner gate is unreachable from any step below.**
  `agent pty` creates `{"agent": <provider>, "mode": "single"}`, and the
  consensus projector that #835 gates spawns **only for `SessionMode::Shared`**
  (`bin/noded/src/term.rs`). **No production surface creates a Shared session** —
  the only way in is a hand-rolled `POST /v1/term/sessions {"mode":"shared"}`
  plus a second member posting into the `term-<session_id>` chat channel. So the
  gate is real, its refusal token is `command_not_channel_owner` (DEBUG, target
  `ducktape::term`), and **T1-8 asserting it would be an assertion that always
  passes.** Exercising it needs a new step that does not exist; if it is wanted,
  write it as one, do not bolt it onto the pty step.
  (Doc drift while here: `2026-07-26-work-admission.md` still describes this
  fix as a `MembersOnly` post policy. #835 explicitly rejected that route —
  chat's `SetMembership` drops its `author`, so any member could add themselves
  — and gated at `project_message` instead. Worth correcting in that doc.)
- **The three `#[ignore]`d provider tests still skip silently**
  (`crates/services/provider/src/lib.rs`:
  `podman_socket_interactive_session_drives_a_tty`,
  `podman_socket_echo_round_trips_through_invoke`, `macos_tart_hardware_smoke`).
  A default run reports them `ignored`, which is honest. They only lie under
  `cargo test -- --ignored` on a host without `DUCKTAPE_PODMAN_SOCKET` or off
  Apple Silicon. **Do not run this pass with `--ignored`**, and if you do, read
  their stdout.

---

## 1. Preconditions (P) — 9 steps

### P-1 — the two environment traps, closed explicitly
**Run on:** both boxes, **in every shell** that starts a node or a daemon.

`pasta` and `crun` live off the default `PATH`; `nft` is in `/sbin`. A bare
`which pasta` says "missing" and it is not (§0.3).

`SandboxBackend::probe()` (`crates/services/sandbox/src/sandbox.rs`) requires
**podman + pasta + nft + nsenter**. `find_on_path` searches `PATH` only;
`find_system_tool` (`podman_api.rs`) additionally searches `/usr/sbin`, `/sbin`,
`/usr/bin`, `/bin` — so `nft` and `nsenter` resolve without help, and **`pasta`
does not**.

```bash
# dev box — put this in EVERY shell that runs a node, a daemon, or cargo test
export PATH="$HOME/.local/opt/podman-debian13/root/usr/bin:$PATH"
export TMPDIR="$HOME/wave2qa-tmp"     # §0.3 — disk-backed, outside any worktree
```

**Observable / pass:** all five resolve, and each prints a real path:
```bash
for t in podman pasta crun; do printf '%-8s %s\n' "$t" "$(command -v "$t" || echo MISSING)"; done
printf '%-8s %s\n' nft     "$(command -v nft || ls /sbin/nft)"
printf '%-8s %s\n' nsenter "$(command -v nsenter)"
df -h "$TMPDIR" | tail -1     # MUST NOT say tmpfs
```
**Fail:** any `MISSING`, or a tmpfs `TMPDIR`.

> **[#831] This is now load-bearing for the test suites too.** A missing tool
> used to make a suite skip silently and report green. It now **panics**. So a
> shell that skipped this export turns every sandboxed suite red with
> `<test> cannot run on this host: pasta is not runnable on PATH`. That is the
> intended behaviour and it is a **FAIL of P-1**, not of the suite. Do not
> reach for `DUCKTAPE_ALLOW_MISSING_TOOLS=1` to make it go away — see P-9.

### P-2 — build the tree
**Run on:** both boxes.

There is no integration branch. Everything wave-2 is in `dev`.

```bash
cd /home/eddy/dev/ducktape
git worktree add .worktree/wave2-qa -b qa/wave2-integration origin/dev
cd .worktree/wave2-qa
git rev-parse HEAD                       # MUST be fc6334d8f (or later dev)
cargo build --release -p node-bin --bin ducktape
```

Worktree location is mandated by `CLAUDE.md` (`<primary-checkout>/.worktree/<slug>`,
never `/tmp` — it is memory-backed, §0.3). **`TMPDIR` must point outside it**
(§0.3) — a scratch dir inside a worktree is one `git add -A` from committing a
key.

**Observable / pass:** a `release/ducktape` binary exists AND identifies itself:
```bash
./target/release/ducktape --version
git -C . rev-parse HEAD          # record this; it is the pass's SHA
git status --porcelain | head    # MUST be empty — a dirty tree changes the build stamp

# take V-2's route baseline NOW, from the same tree that built the binary
LOGS="$HOME/wave2qa-logs"; mkdir -p "$LOGS"
grep -oE '\.route\("(/v1[^"]*)"' bin/noded/src/lib.rs | sed 's/.*("//' | sort \
  > "$LOGS/routes.baseline"
wc -l < "$LOGS/routes.baseline"   # 33 on this tree — record it
```
**Fail:** a non-empty `git status` (a dirty tree makes `DUCKTAPE_BUILD` a
working-tree digest, which will read as skew in R-3 for the wrong reason).

**macmini:** same commit, native ARM build. It has no `cargo` by default —
rustup was installed there previously. Binary at
`~/dev/ducktape/target/release/ducktape`.

### P-3 — confirm both boxes are on the same build
**Run on:** both.

`DUCKTAPE_BUILD` is stamped at compile time by `bin/noded/build.rs` from the
commit plus a working-tree digest when dirty. It is `option_env!`, so **setting
it at runtime does nothing**.

**Observable / pass:** both boxes print the same SHA from P-2, and
`ducktape service status` later shows `build` with no `(this node: …)` suffix
(§R-3). **Fail:** different SHAs — fix before proceeding; every later skew
assertion becomes meaningless.

### P-4 — reap the box's leaked state: processes, sockets, **and storage roots**
**Run on:** dev box.

**This step used to lie.** It reaped processes and sockets and called the box
clean, while leaving every test's **storage root** on the tmpfs. Both halves
matter, and the second one is the half that fills `/tmp` (§0.3).

**a. processes.** The PR test suites launch `podman system service --time=0`,
which never idle-timeouts; 102 orphans parented to `init` were counted on this
box in one day. Reap by verified identity, never by pattern:

```bash
ls /run/user/1000/ducktape/ | wc -l
pgrep -u "$USER" -f -c 'podman.*system service'     # count only — do NOT kill by pattern

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

**b. storage roots — and plain `rm -rf` CANNOT remove them.**

Rootless podman chowns its overlay graph root into a **user namespace** (the
subuid range). The unmapped user cannot traverse or unlink those directories, so
`rm -rf` **walks what it can, removes that, and exits 0** — reporting success
while leaving the bulk behind. Measured on this campaign: **~830 MB of an
845 MB store survived a "successful" `rm -rf`.**

The command is:

```bash
podman unshare rm -rf <path>
```

`podman unshare` re-enters the same user namespace the storage was created in,
where those directories are owned by root and removable. Sweep the leftovers:

```bash
# the graph roots the suites leave behind: <data>/podman/{storage,run,hooks}
for d in /tmp/.tmp*/ /tmp/dt-svc-check/; do
  [ -d "$d" ] || continue
  podman unshare rm -rf "$d"
done
```

**Observable / pass — a post-state, looked at, not an exit code:**
```bash
pgrep -u "$USER" -f -c 'podman.*system service'   # -> 0
ls /run/user/1000/ducktape/ | wc -l               # -> 0
ls -d /tmp/.tmp*/ 2>/dev/null | wc -l             # -> 0
du -csh /tmp/.tmp* /tmp/dt-svc-check 2>/dev/null | tail -1   # -> nothing, or 0
df -h /tmp | tail -1                              # record the free figure
```
**`rm -rf` exiting 0 is NOT the pass.** Re-run `du` and read the number.
**Fail:** a survivor whose cmdline points at a **workspace** (not `/tmp`) — that
is a live node's service; investigate, do not kill.

**c. the same shape, everywhere else in the teardown.** Every step that destroys
state must name what it looks at afterwards. Swept for this revision:

| step | was | now |
|---|---|---|
| **K-1** ("make the image store genuinely empty") | `rm -rf "$WS/…/podman/storage"` — **the same rootless-overlay path**, so the store was never emptied and the cold-start step proved nothing | `podman unshare rm -rf`, then `du -sh` the path and assert the image list is empty |
| **I-3** (airlock SIGTERM) | already asserts the route is **gone** from `gateway-routes.json` and the file **removed** if it was the only one | kept — this one was already a post-state |
| **I-4** (SIGKILL blast radius) | "poll for the state string" | kept, and the container list is re-read on both sockets |
| **V-2** (`/v1` additive only) | a `git diff origin/dev...HEAD` grep that is **structurally empty on this tree** | replaced with a route inventory — see V-2 |
| **V-3** (no secrets in logs) | greps expecting empty output | now asserts the log files are **non-empty first**, so a missing log cannot pass |

### P-5 — workspace layout, and the node config
**Run on:** both.

Registry root is `$DUCKTAPE_HOME/workspaces` when set, else
`~/.ducktape/workspaces` (`config::workspaces_root`, `bin/node/src/config/mod.rs`).
Use `DUCKTAPE_HOME` to keep the pass off the user's real workspaces:

```bash
export DUCKTAPE_HOME="$HOME/.ducktape-wave2qa"
```

Layout after init:

```
$DUCKTAPE_HOME/workspaces/<CHAIN-ID>/
  node.toml                 # the config
  network.toml              # chain_id, validators, reach
  identity.key              # node ed25519 secret, 0600
  services.toml             # THE GRANT FILE — created by `service enable`, deleted when empty
  service-link.token        # 32B hex, 0600, minted EVERY boot. ALSO the ws topic secret [#827]
  admin.token               # 64 hex chars, 0600, minted EVERY boot
  work-admit.toml           # THE ADMISSION FILE — created by `node work admit`  [#838]
  gateway-routes.json       # port-scoped local routes
  daemon.log                # the NODE's stderr tee (NOT the daemons' — see P-7)
  storage/
    services/compute/podman/{storage,run,hooks,owner.pid,podman.pid}
    services/agent/podman/{storage,run,hooks,owner.pid,podman.pid}
    airlock-creds/{<name>/,seal.key}
    agent-workspaces/  agent-sessions/  term-sessions/  forge-repo/
  agent-runs/<16-hex salt>/     # SIBLING of storage, not inside it
```

The two `podman/storage` dirs are the rootless overlay roots from P-4b: **they
need `podman unshare` to delete, here and in K-1.**

Podman sockets are **not** under the data dir (108-byte `sockaddr_un` cap):
```
${XDG_RUNTIME_DIR}/ducktape/ducktape-<fnv1a32 of the data dir, 8 hex>-compute.sock
${XDG_RUNTIME_DIR}/ducktape/ducktape-<same tag>-agent.sock
```
The tag is a hash of the **data dir path**, so it changes if the workspace
moves. Graph root and runroot are keyed by **kind**; the **instance id appears
in neither** — it appears only in the container label (C-1).

**`services.toml` is still a local file, and the grant still lives only there.**
#829 moved the *announce* on chain, not the grant: nothing in `CapabilityMsg`
carries an instance id. Grant = local; announce = on-chain; two different facts.
One consequence to know: `Services::validate` now runs the announce-set
computation on every `load`, so a corrupt or over-cap `services.toml` **fails
the node boot**, not just the announce.

### P-6 — the `[sandbox]` table (the second trap)
**Run on:** both.

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

`ducktape service run <kind>` calls `noded::log::init(None, None)`
(`bin/node/src/services.rs`) — **stderr only, no file, no log ring.** Only
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

Gated routes, the full list (`bin/noded/src/admin.rs`, five of them):
`GET /v1/admin/ping`, `POST /v1/admin/shutdown`, `GET /v1/admin/logs/tail`,
`POST /v1/admin/module-code/stage`, `GET /v1/admin/module-code/{digest}`.

> **`/v1/log-filter` is NOT gated.** It is on the public router
> (`bin/noded/src/lib.rs`). `CLAUDE.md`'s
> `curl -XPOST localhost:$PORT/v1/log-filter -d '…'` keeps working with no
> header. Do not "fix" it.

> **THE TRAP — the token is not always sufficient.** `admit_gate`
> (`bin/noded/src/admin.rs`) first resolves the node's owner from the
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
> **Consequence for `ops/demo-clear.sh`:** it sends the token, but on an *owned*
> node that request gets `403 not_the_owner`; the script falls through to its pid
> sweep, so it still works — but do not read its success as proof the admin gate
> accepted anything. Record this as a real (small) gap.

**Never paste the token or its file contents into a report.**

### P-9 — the anti-silent-skip discipline
**This is the step the whole runbook hangs on. #831 changed its polarity.**

The campaign was burned by a suite that "passed" because every daemon exited at
boot. **#831 inverted the default tree-wide, and the two hazards this step used
to name are fixed.**

**What #831 actually did** (`crates/testing/nettest/src/lib.rs`, `skip_without`
/ `decide_skip`):

- a missing capability **panics** by default:
  `<test> cannot run on this host: <why> — Install what is missing, or set
  DUCKTAPE_ALLOW_MISSING_TOOLS=1 to skip instead — and then do not read this
  suite's green as coverage.`
- `DUCKTAPE_ALLOW_MISSING_TOOLS=1` is the **only** way to get a skip, and it is
  a deliberate act;
- when it does skip, the line is written to **fd 2 directly**, not via
  `eprintln!` — because the macro routes through `std::io::_eprint`, which
  honours libtest's thread-local capture and **would swallow the line on a
  passing test**. That swallowing is the exact defect the helper exists to kill.

The helper's own doc names the cost of the old default: *"that is how five
forge-over-http tests, a whole compute plane and a claim lane each spent weeks
proving nothing."*

**Verified at `fc6334d8f` — the old hazards are closed:**

| suite | old claim | now |
|---|---|---|
| `bin/node/tests/dispatch_e2e.rs` | two bare `return`s reporting `ok` | routed through `skip_unless_sandboxed` → `nettest::skip_without`. **Panics** unless the env var is set. |
| `bin/node/tests/dogfood_loop_e2e.rs` | missing-`[sandbox]` defect, dead on dev | routed through the same helper |
| `bin/node/tests/sched_pinned_run.rs` | weaker `podman version` predicate | routed through the same helper — which asks the product's own `SandboxBackend::probe()`, not `podman version`. Its module doc now says *"a captured 'skipping' line is not a signal anyone sees"* |

Same treatment reached `remote_session.rs`, `portable_workspace_e2e.rs`, and the
five git tests in `bin/noded/tests/daemon_e2e.rs`. The commit that did it states
the intent plainly: *"`cargo test --workspace` on a box without podman/pasta/git
is now RED. That is the intent."*

**Three silent skips survive, and all three are `#[ignore]`d** — see §0.4b. They
are honest under a default run and only lie under `--ignored`.

**A second class of vacuum #831 closed, worth knowing:** several suites were
building integration binaries containing **zero tests** — `cargo test -p airlock`
built three such binaries (now 13, 8 and 4 tests), `node --test submit_decoded`
went 0→2, `consensus` built 7 of 8 targets, `nat-traversal` did not compile.
A suite that runs zero tests reports `ok` too.

**So the discipline inverts.** The thing to police is no longer a silent skip —
it is **someone setting the escape hatch to make a red box green.**

```bash
# before ANY cargo test in this pass:
env | grep -c DUCKTAPE_ALLOW_MISSING_TOOLS      # MUST be 0

cargo test -p node-bin --test dispatch_e2e 2>&1 | tee dispatch.out
grep -c '^SKIP ' dispatch.out                   # MUST be 0
grep -c 'compute daemon serving' "$LOGS"/*.log  # MUST be >= 1
```

**If a suite panics with `cannot run on this host`, that is a FAIL of P-1, not
of the suite, and not a reason to set the variable.**

**The hazard that is NOT fixed, and is silent by construction.** A malformed
`Create` frame is dropped by the agent daemon with the serde message
**discarded** (`Err(_) =>` in `classify`, `bin/node/src/agent/link.rs`), one WARN
`reason="malformed_command"`, and the ws link stays attached and
healthy-looking — while the node's `TermSessions::start` awaits the reply with
**no timeout at all** (`bin/noded/src/term.rs`, deliberate: "a cold image pull
legitimately takes minutes"). So the symptom is *a session create that never
returns and never errors*. **Any step that creates a session must treat "no
answer" as a FAIL with that grep, never as slowness.**

**Liveness before probe, always.** `await_status`
(`bin/noded/tests/daemon_e2e.rs`) checks `child.try_wait()` **before** probing
the port, because a node that lost a port race exits — and a probe-first loop
then adopts *a stranger's* 200 as its own readiness and drives someone else's
node for the rest of the run. Its panic says it outright: *"If something still
answers that port, it is NOT ours."* Mirror that here: before every
`await_published`, confirm the pid you started is still alive.

**Rule for every step below:** a step reports **PASS**, **FAIL**, or
**EXPECTED-REFUSAL** — never blank, and (since every wave-2 PR has merged)
**never SKIPPED for a PR reason**. A tool that is absent is a **FAIL of P-1**.

---

## 2. Tier 1 — no airlock at runtime (T1) — 8 steps

The claim under test: **airlock is a credential SOURCE, not a dependency.**
broker-host is always in the path (per-run loopback + opaque bearer); the
operator's own credential resolves locally and never touches an airlock.

Topology for this tier: **dev box alone.** One node, both daemons.

### T1-1 — found the network, node up

```bash
export DUCKTAPE_HOME="$HOME/.ducktape-wave2qa"
export PATH="$HOME/.local/opt/podman-debian13/root/usr/bin:$PATH"
export TMPDIR="$HOME/wave2qa-tmp"
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
the reason in §0.1. `app surface listening` in `daemon.log` marks the *bind*,
which is strictly earlier than the first publish and must not be used as the
gate.

**Observable / pass, all five:**
- `daemon.log` carries `INFO ducktape::node: node boot node=<hex8> version=… binary=… built_unix=…`
- `daemon.log` carries `INFO ducktape::consensus: … genesis root_hash=<64 hex>` — **record this hash; it is the V-1 cross-box anchor**
- `daemon.log` carries the `reason="compute_not_granted"` warn from P-6
- `test -s "$WS/admin.token" && test -s "$WS/service-link.token"` — both exist, both `0600`
- after `await_published`, `/v1/status` carries a non-empty `public_key`, a real
  `version`, and a real `root_hash`:
  ```bash
  curl -s "http://127.0.0.1:9971/v1/status" | head -c 400
  ```

**Record, do not fail on:** the size of the pre-publish window, from
`boot::surfaces::bind` to the first `status.publish` in `bin/node/src/main.rs`.
Measured on `ducktape-noded` before #821 it was ~1.3 s. **Worth timing here and
reporting** — it is the window T1-2's exit lands in, and it is the one of the
three node binaries still open (§0.1).

### T1-2 — compute signals, and is refused nothing

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
- **the shortened fuse.** `node_identity()` is the *first* hard dependency on a
  live node — earlier than `send_hello`, by one whole `backend.probe()` (which
  on a cold host takes seconds). There are **zero retries**: the first failure
  propagates and `bin/node/src/main.rs` prints `FATAL: <err>` to **stderr** (not
  tracing) and `exit(1)`. Two distinct lines, and **the second is misleading —
  flag it as a finding**:

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
- HTTP 409 `build_mismatch` on the hello → **impossible in this tree**; #820
  deleted both build refusals. Seeing one means the binary is not from P-2.

### T1-3 — the consent boundary is now a transaction
**Changed by #829 — this is the most rewritten step in the runbook.**

`service enable` is no longer a local file write. It resolves the node's
identity over `/v1/status`, checks the daemon is signaling **right now**, mints
the grant, persists `services.toml`, **and then submits a
`capability::CapabilityMsg::Announce` transaction and blocks until it
finalizes.** The CLI holds no key: `POST /v1/submit` discards the caller's
claimed origin and the node re-signs with its own signer
(`bin/node/src/validator/run/ingress.rs`), which is what "node-signed, keyless
CLI" means. There is a source-parsing lint test on that property —
`the_daemon_path_cannot_name_the_node_key` (`bin/node/src/services.rs`).

```bash
INSTANCE=$("$D" service enable compute -n "$CHAIN" -y)    # stdout is the id ALONE
echo "$INSTANCE"                                          # -> compute#xxxxxxxx
```

**Observable / pass:**
- stdout is exactly `compute#<8 hex>` and nothing else (so `$(...)` is
  scriptable). This survived #829 — the `println!` is the only stdout write in
  the verb.
- **stderr now reports a height**: `✓ enabled compute#… · announced at height <N>`.
  **That height is the step's real observable** — it is the proof the consent
  reached consensus rather than a file.
- the consent summary (red-painted `service / node / status / offers / grant
  scopes`, `status` = green `signaling`) precedes it on stderr
- `$WS/services.toml` now exists, `version = 1`, one `[[service]]` with
  `kind = "compute"`, `instance` = 64 lowercase hex, `nonce` = 32 lowercase hex
- `"$D" service status -n "$CHAIN" --json | python3 -m json.tool` gives **9**
  keys: `kind, state, instance, version, build, capabilities, scopes, needs,
  unmet_needs`, with `state: "enabled"`.
  Note `service list --json` emits the same shape but the **table** shows only
  `KIND / STATE / INSTANCE`; `build` is a `status`-only row label.

**The settle is bounded, and the failure is partial — assert both.** `SUBMIT_HOLD`
is **10 s** (`bin/node/src/constants.rs`). Past it the node answers
`400 {"error":"timed out awaiting finalization — re-query on the next block"}`.
The CLI wraps every submit failure identically:

```
compute is granted but NOT announced, so nothing will be placed on it yet — this node retries every 10s until it lands: <inner>
```

**This is a persist-then-submit verb: on failure the grant IS already on disk**
and `service status` shows the kind granted, while the announce is absent. The
verb still exits nonzero and prints **nothing** on stdout — so `INSTANCE` comes
back empty rather than half-set. Assert that: an empty `$INSTANCE` with a
nonzero exit is the correct shape of this failure.

**Deliberate-failure half — cheap, and it proves the transaction is real.**
Stop the node, then run `service enable agent -y`. It must fail with
`could not read this node's identity: the node is not running` **before** any
consent prompt. A version of `enable` that still worked with the node down
would be the old local-file write.

**Assert the negative:** `"$D" service enable broker -n "$CHAIN" -y` must fail
with `broker is not signaling to this node, so there is nothing to consent to
— start it first: ducktape service run broker`. **broker and sandbox are
libraries, never enable-able services** — this is the plan's own litmus.

> Note: `<KIND>` is **not** a closed enum. Any `1..32 chars of [a-z0-9-]` is
> accepted, signals, and executes nothing (`daemon_for` returns `None` →
> `Served::SignalOnly` → the process parks). So the assertion above proves
> "there is no broker daemon", not "the CLI rejects the word broker".

### T1-4 — the compute daemon actually serves

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
  The claim carries **no `build`** (#820 deleted the field); the node compares no
  stamp. Confirm no `link_refused` in `agent.log`.
- two podman services now, two graph roots:
  ```bash
  ls "$WS/storage/services/"                     # agent  compute
  ls "$XDG_RUNTIME_DIR/ducktape/"*-agent.sock "$XDG_RUNTIME_DIR/ducktape/"*-compute.sock
  ```

> **[#829] the auto-enable path does not die on a failed announce.** `service run
> --enable` downgrades a failed commit to a warning with
> `reason="enable_not_announced"` and keeps signaling. So a green
> `agent daemon serving` does **not** by itself prove the announce landed —
> grep for that reason token before trusting T2-3.

**Fail:** `reason="link_refused"` with
`refused: present this node's service-link token, and only one agent service may
attach` → the token was stale (the node restarted) or a second agent is running.

### T1-6 — an operator-owned credential, no airlock in the path

> **HOLD HERE. The credential is supplied by the user, not scavenged.**
> This step needs a **throwaway** vendor credential that the user provides at
> the time of the run. **Do not read the operator's live
> `~/.claude/.credentials.json`** — the pass may rotate or invalidate it, and a
> run that burns the user's real login is not a test result.
>
> The runbook **stops at this step** until that credential is in hand. Record it
> as `BLOCKED(awaiting user-supplied throwaway credential)` if the pass reaches
> here without one. It is the only manual gate in the document.

```bash
"$D" user account-init --name eddy -n "$CHAIN"     # password on stdin
"$D" user cred add claude -n "$CHAIN"              # browser OAuth, on the throwaway
```
Headless alternative, **pointed at the user-supplied artifact only**:
`DUCKTAPE_CRED_REUSE_ARTIFACT=<path to the throwaway .credentials.json>`
imports it instead of driving the browser.

**Observable / pass:**
- `$WS/storage/airlock-creds/<name>/` exists with a `kind` marker and the vendor
  artifact; `$WS/storage/airlock-creds/seal.key` exists at **0600**
- `cred add` **auto-published** the on-chain `airlock` RouteStatement — no
  hand-built JSON
- `cred add` prints `lend it by running: ducktape service run airlock`
- the node's boot warn `reason="airlock_not_granted"` (target
  `ducktape::service`, carries `credentials=<count>`) appears on the **next node
  restart** — the store is now non-empty and no airlock grant exists. It is a
  warn, not a refusal; the node keeps serving.

> **From here the node has a committed owner.** Every later `/v1/admin/*` call
> needs the owner PoP, not the token (P-8).

### T1-7 — headless run, end to end, airlock-free
**The ws leg changed under #827 — the old subscribe frame is now refused.**

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
3. tail the live output ring. There is **no CLI verb** — this is the ws topic,
   and **`run-output:` is now `Admission::Workspace`**
   (`Topic::admission`, `bin/noded/src/stream.rs`). The subscribe frame must
   carry the workspace secret:
   ```json
   {"op":"subscribe","topics":["run-output:<DISPATCH>"],"token":"<contents of $WS/service-link.token>"}
   ```
   ```bash
   TOKEN="$(cat "$WS/service-link.token")"   # 0600; never paste it into a report
   ```
   `ClientMsg` is `deny_unknown_fields`, so the field is spelled `token` and
   nothing else. The same secret `ServiceAttach` presents, deliberately.

**Observable / pass:**
- `recent_runs` carries this run with a success outcome and the model's `PONG`
- **the same `PONG` crossed the run-output ring** — this is the positive half,
  and it is what stops the step passing on the negative grep alone
- a real container ran: a container labelled `io.ducktape.managed=compute#…`
  appears on the compute socket during the run (C-1's query)
- **the airlock daemon was never started and nothing 502'd**: `grep -cE
  'airlock_gateway_unreachable|airlock_route_or_credential_absent|gateway_seal_pk_mismatch|airlock_caller_account_unverified'
  "$LOGS"/*.log` → **0**. *This is the tier's whole point.*
- `"$LOGS/compute.log"` has **no** `reason="worker_error"`

**Deliberate-failure half — the topic gate, in one frame.** Send the identical
subscribe with the `token` field **omitted**.

**Pass** — the error frame, read explicitly:
```json
{"type":"error","topic":"run-output:<DISPATCH>","code":"forbidden",
 "detail":"this topic requires the node's service-link token — read it from the workspace and send it as `token` on the subscribe"}
```
plus one `debug` line, target `ducktape::stream`, `reason="topic_not_admitted"`.
The detail is a `&'static str` by construction, so no secret can reach the
formatter.

> **THE TRAP, and it is the reason this half exists.** A
> `{"type":"subscribed","topics":{…}}` frame is **still sent** after the
> refusal, with the refused topic simply **absent from the map**. A client that
> waits for `subscribed` and calls it success **sees a false green.** The error
> frame is the only signal. **PASS requires reading the `error` frame; the
> presence of `subscribed` proves nothing.**

**Fail:** the topic appears in the `subscribed` map — the gate is not wired.

Two sibling refusals worth one frame each: an unknown topic name →
`reason="unknown_topic"`; a real family naming a module this node does not index
→ `reason="unknown_module"`. They are separate tokens on purpose ("one token
covering both would be uncountable"), and both carry wire code `unknown_topic`,
distinct from `forbidden`.

> **A related fix worth confirming while you are here.** `fresh_dispatch_id`
> (`bin/node/src/agent_cli.rs`) was 16 random bytes; `RUN_OUTPUT_ID_LEN = 64`
> hex chars, so **every `agent sched` run's live output was silently dropped**
> with `reason="malformed_run_id"`. It is 32 bytes now. If the ring stays empty
> on a run that otherwise succeeds, grep for that token before suspecting the
> gate.

**Fail:** a 404-shaped container-create failure → the libpod pull-on-404 path is
broken (it is in this tree; #826 merged).
**Fail:** any `airlock_*` reason token → airlock is on the runtime path, which
contradicts the design claim. Report it as the headline finding.

### T1-8 — interactive pty session, airlock-free

```bash
"$D" agent pty claude -n "$CHAIN" --cpu 1 --mem 2
```

> **[#827] `agent pty` reads the workspace secret before it creates a session.**
> `let secret = workspace_secret(addr)?;` is the **first** statement of
> `cmd_pty` (`bin/node/src/agent_cli.rs`), ahead of `resolve_provider` and
> `create_session` — in the code's words, *"failing here costs no container."*
> It resolves the workspace on the same `NodeAddr` ladder that resolved the http
> base and reads the 0600 `service-link.token` (not `admin.token`, not a new
> file), then presents it on the `term:<session_id>` subscribe.
>
> **A pty therefore requires read access to a LOCAL workspace** — it is not a
> pure network client, and there is no way to pass the token on the command
> line. Exact failures, all `FATAL:` + exit 1:
>
> | failure | string |
> |---|---|
> | workspace not resolvable | `FATAL: attaching a pty needs this node's workspace: <why>` |
> | `<why>`, none registered | `no registered workspace serves <base> — name it with -n/--network <chain-id>` |
> | `<why>`, several | `several workspaces serve <base> — pick one with -n:` + the list |
> | file unreadable | `FATAL: secret file <workspace>/service-link.token: <io error>` |
>
> **The secret is the SUBMITTING box's own, not the executor's** — the pty
> output fans back onto the *guest* node's `term:<id>` topic. So T2-6 run from
> the dev box needs the **dev box's** token, and does **not** need the
> borrower's. This is the opposite of the intuition and worth stating in the
> report.

**Observable / pass:**
- stderr prints `attached to <session_id> (term:<session_id>)`
- a real TUI renders; typing echoes; a terminal resize propagates (SIGWINCH →
  `{"op":"term_resize",…}`)
- the session ends cleanly on provider exit — the node emits a
  `{"type":"term_ended",…}` frame and the CLI shuts down its background reader
  (this is the #779/#780 wedge fix; a hang here is a regression)
- `agent.log` shows the `TermCreate` decode succeeded

**No #835 assertion here — see §0.4b.** The shared-pty owner gate is real, but
`agent pty` creates a `single`-mode session and the gate only runs for `Shared`.
Asserting it in this step would be an assertion that always passes.

**[#820] the deliberate-failure half.** `Create` is `deny_unknown_fields` with
`limits` and `credential` **required**. Prove the gate refuses, by hand-driving
one malformed frame at the agent daemon's ws link. The minimal valid frame:
```json
{"op":"term_create","session":"a","provider":"claude","restricted":false,"limits":{},"credential":null}
```

| frame mutation | must be refused because |
|---|---|
| omit `"limits"` | `missing field \`limits\`` — previously `#[serde(default)]` → empty map = "provider defaults" |
| omit `"credential"` | `missing field \`credential\`` — an `Option` normally decodes absent → `None` = "the operator's own credential". `#[serde(deserialize_with = "Option::deserialize")]` (`wire.rs`) is what suppresses that fallback. **This was the widest fail-open: silence granted authority.** |
| add `"spend_cap": 5` | `unknown field \`spend_cap\`` |
| add an unknown key to a `ClientMsg` | `BadFrame` naming the field |

> **The observable is NOT an error message — read this before running it.**
> `Create` flows node → daemon, and the daemon's `classify`
> (`bin/node/src/agent/link.rs`) matches `Err(_)` and **discards the serde
> text**. What you actually get:
> - one WARN, target `ducktape::service`, `reason="malformed_command"`, message
>   `agent daemon dropped a frame it could not decode` — **no field name, no
>   session, no detail**
> - the ws link stays **attached and healthy** (`Incoming::Ignore`, read loop
>   continues)
> - the node's `TermSessions::start` **hangs forever** — there is deliberately
>   no timeout (`bin/noded/src/term.rs`), because a cold image pull legitimately
>   takes minutes. It is released only when the daemon detaches.
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

## 3. Tier 2 — credential lending through the airlock daemon (T2) — 7 steps

Cross-box. **Lender = dev box** (holds the credential, runs `service run
airlock`). **Borrower = a Linux/podman box** (runs compute/agent, executes the
run) — see T2-6 for why the borrower is Linux and not the macmini.

> **This tier is the one that has never actually run.** The cross-box terminal
> lane has **unit and behavioural coverage only — no live two-box pass, ever.**
> T2-5, T2-6 and X-1a/X-1b are the steps that would prove it, which makes them
> the highest-value steps in this document regardless of where they sit in the
> ordering. Each of them therefore carries a deliberate-failure half, and none of
> them may be recorded as PASS on the absence of an error.

### T2-1 — found a FRESH chain, and clean the legacy workspaces first

**Found a new chain for this pass. Do not plan around reusing an old one.**
`dukenet#03f6df3d` from the 2-node campaign is **gone from the dev box** — the
workspace no longer exists and is not being restored. Verified at revision time:
`~/.ducktape/workspaces/` is **empty** on the dev box.

```bash
# both boxes, BEFORE founding: inventory and clear the legacy workspaces
ls -la ~/.ducktape/workspaces/         # dev box: expected empty
"$D" node list                         # the registry's own view
```
Clean any survivor with its own teardown (stop the node by verified identity,
then remove the workspace and drop it from the registry) — **and remember its
`storage/services/*/podman/storage` needs `podman unshare rm -rf`** (P-4b), or
you will leave gigabytes and a half-deleted graph root behind.

The tailnet is live — `zk` `100.76.154.57`, `macmini-duke` `100.110.104.117` —
and LAN between them is dead (same NAT, client isolation, hairpin blocked), so
**tailnet only**, `primary_coordinator = "none"`.

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

```bash
# dev box (lender)
RUST_LOG=info,ducktape::gateway=debug \
  "$D" service run airlock -n "$CHAIN" --enable \
  > "$LOGS/airlock.out" 2> "$LOGS/airlock.log" &
```

**Wait on:** `await_line "$LOGS/airlock.log" 'airlock daemon serving'`
**Observable / pass, in this exact order** (`bin/node/src/airlock.rs`):
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
is correct: a lending laptop has no container runtime, and airlock runs **no
sandbox probe**. (And per §0.4, those scopes gate nothing — they are a label.)

**Assert the heartbeat.** Hand-unbind the route and watch it come back:
```bash
"$D" gateway unbind --label airlock -n "$CHAIN"
# poll `gateway list` until the route RETURNS — reassert re-registers a Vacant slot
```
The beat is `HELLO_TTL/3` = **10 s**, logged on beat 1 then every **30th**
(`LOG_EVERY = 30` — note: **not** 32; 32 is #819's two unrelated constants).

**Assert the lender serves no upload route.** `POST /credential` is **not
mounted** on a self-host lender build — only the attested build mounts it. A
`404` here is the pass, pinned by
`the_self_host_lender_serves_no_credential_upload` (`crates/airlock/tests/e2e.rs`).

### T2-3 — the kind is discoverable on chain

```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":{"providers":{"capability":"airlock"}}}'
```
**Observable / pass:** the lender's node key is in `providers`. An airlock grant
carries **zero executor tags**, so it is discoverable only because the announce
inserts the *kind* itself — the property `#819` added and
`a_kind_with_no_executors_still_announces_itself` pins. Without it a green
airlock is invisible to every peer.

Also check the borrower announces its kinds:
```bash
curl -s http://<borrower>:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":"all"}'
```
**Pass:** the borrower's row contains `compute` and `agent` as tags in their own
right, alongside executor tags. **There is no CLI verb for this** — `service
list`/`status` read the *local* catalog, never the committed registry.

> **[#829] where the announce comes from now.** The validator announce pump is
> **deleted** (`bin/node/src/validator/announce.rs` no longer exists). Two
> triggers, one writer (`bin/node/src/announce.rs`): the **verb** announces
> synchronously and reports a height (T1-3), and a **liveness watcher** thread
> re-reads `services.toml` every `TICK` = 10 s as the backstop. The watcher
> **submits nothing for the first `SETTLE` = 30 s after node start**, and an
> unanswered `/v1/query` is `Tick::Unknown` and never retracts. So: if this
> query is empty right after a node restart, **wait for the settle before
> calling it a failure** — poll for the transition, do not conclude on the first
> read. Its failure log is `reason="announce_failed"` with `attempts`, first
> then every 32nd; its success marker is `"capabilities announced"` with a
> `height`.

### T2-4 — grant the credential to the BORROWER's account
**Changed by #833 — the direction is the opposite of what it looks like.**

```bash
# borrower box
"$D" user account-init --name duke -n "$CHAIN"
# dev box (lender)
"$D" user cred grant eddy-claude-1 <borrower-account-hex> -n "$CHAIN"
```

**The grant names the account of the node that will RUN the workload — not the
account that submits.** Since #833 the credential path has no caller-asserted
account: the drawing identity is `x-duck-caller-account`, minted by the node's
gateway proxy from the mesh-verified WireGuard peer, and a caller-supplied
`x-duck-*` header is refused at the proxy's own decode. So the lender sees the
**executor**. Granting the submitter's account accomplishes nothing — see §0.4a.

**Observable / pass:**
- the `gateway` module's credential record lists the **borrower's** account as
  grantee, and `cred grant` reports `granted at height <N>`
- **Do not assert that any scope is enforced** (§0.4).

**Deliberate-failure half, and it is the delegation proof.** Before granting the
borrower, grant the **lender's own** account instead and run T2-5. It must fail
with `credential_not_granted` even though the credential's owner submitted.
Record `EXPECTED-REFUSAL` and cite §0.4a. This is the single cheapest way to
prove the caller-asserted account is really gone.

### T2-4b — admit the submitter's work on the EXECUTING node

**A credential grant and a work admission are two consents in OPPOSITE
directions, and conflating them is the first thing to get wrong.** T2-4 is the
LENDER saying *which account may draw on my credential*. This step is the
EXECUTOR saying *whose work I will run at all*. A cross-node run needs both, on
different boxes — the grant on the dev box, the admission on the borrower.

Since #838 a node runs only its OWNER's work and its own by default, and these
two boxes are two accounts (`eddy` on the dev box from T1-6, `duke` on the
borrower from T2-4). So without this step T2-5, T2-6 and X-1 all fail —
deliberately, and each has a deliberate-failure half below that proves it.

```bash
# borrower (the EXECUTOR) — the dev box's account, not the credential owner's
"$D" node work admit <dev-box-account-hex> -n "$CHAIN"
"$D" node work list -n "$CHAIN"
```

**Observable / pass:**
- `node work list` prints `owner, plus 1 admitted account(s):` and the hex
- `$WS/work-admit.toml` exists on the borrower with that one `admit` entry
- **no restart of anything** — the policy is re-read on every decision, by both
  the node (pty lane) and the compute daemon (sched lane)

**Fail:** `no account named …` → pass the hex account id, or a display name the
`identity` module has committed.

### T2-5 — a lent-credential run, cross-box

```bash
# from the dev box, pinned to the borrower.
# NOTE: --host-node, NOT --node.  #832 renamed it; see the trap below.
"$D" agent sched claude --cred eddy-claude-1 --host-node duke -n "$CHAIN" -- "reply with exactly: PONG"
```

> **[#832] THE FLAG TRAP — this one does not fail cleanly.** `agent --node` used
> to mean "which peer runs the work". It now means **an http base url**, and the
> peer-targeting flag is `--host-node`. Passing `--node duke` still **parses**:
> `duke` becomes the top rung of the address ladder, **outranking `-n
> "$CHAIN"`**, and the CLI tries `POST duke/v1/query`. You get a reqwest
> URL-parse error, not "unknown host node", and the name never reaches
> `resolve_host_node` at all. **Any `--node` you inherit from an older recipe is
> silently pointing the CLI at the wrong thing.** The ladder is
> `--node <http-url>` → `-n/--network <chain-id>` → `DUCKTAPE_NODE` → caller
> context → lone registered workspace, and trailing slashes are trimmed on every
> rung.

**Observable / pass:**
- the run executes **on the borrower** and returns `PONG`
- the borrower's broker opened a sealed airlock session: **zero** refusal
  tokens in the borrower's logs
- the lender's `airlock.log` shows the grant gate *deciding*, at `debug`, target
  `ducktape::gateway`, carrying `credential=<name>` and **never** the account
- the run's output crossed the ring on the **borrower's** node (T1-7's gated
  subscribe, with the borrower's own `service-link.token`) — this is the
  positive evidence that stops the step passing on greps alone

**This is the step most likely to catch something.** The failure surface is the
whole point of the refusal taxonomy; §10 maps each token to its cause.

**Deliberate-failure half — run this BEFORE T2-4b.** With the grant in place but
the admission absent, the same command must fail, and fail *loudly*:

```bash
# borrower, before `node work admit`
grep -c 'reason="work_not_admitted"' "$LOGS/compute.log"   # MUST be >= 1
```
**Pass:** the saga reaches `Failed` carrying `work_not_admitted`, and the
borrower's `compute.log` has a WARN with **target `ducktape::saga`** (not
`ducktape::service`), fields `attempt=<(saga_id, attempt)>` and
`reason="work_not_admitted"`, message **`compute attempt refused`** — **once per
attempt**, never once per 15 s tick (an `Entry` is inserted to guarantee that).
And **no container was created** — check the compute socket's container list is
unchanged (C-1's query).
**Fail:** the run succeeds (the admission is not wired), or the saga sits
`Pending` forever with no warn (a silent park — see X-1).

Then run T2-4b and re-run this step: it must now pass, **with no restart of the
node or either daemon** — the policy is re-loaded as the first statement of
`admit()`, which is the sole entry point for both lanes (pinned by the
source-parsing lint `both_lanes_route_through_one_verdict`). That single variable
is the whole point of the step.

> **Submit a NEW run, do not re-poll the refused one.** See X-1b's latch note —
> it applies to the pinned lane's sibling and will waste an hour if missed.

### T2-6 — interactive pty on a lent credential
**The borrower for this leg is LINUX. The macOS case is filed separately.**

```bash
"$D" agent pty claude --cred eddy-claude-1 --host-node duke -n "$CHAIN"
```

> **Why not the macmini.** The blocker is **credential plumbing, not the
> sandbox.** The lent-credential last mile for interactive `claude` on macOS
> needs `HOST_CREDS_FILE` + `PROVIDER_MANAGED_BY_HOST` (commit `b3d95723e`) to
> reach the provider; without them macOS `claude` falls back to the login
> keychain and its TUI gate asks for a login method. **Podman on the macmini
> would not fix it** — the Tart backend is not what is failing. So:
> - **this step runs with a Linux/podman borrower**, where the plumbing exists
>   and the headless token-only path is already proven;
> - **the macOS interactive case is a separate item**, not a KNOWN-GAP verdict
>   inside this step. File it as its own bug against the credential plumbing and
>   name `HOST_CREDS_FILE` / `PROVIDER_MANAGED_BY_HOST` in it.
>
> This removes the step's old escape hatch on purpose. With a Linux borrower
> there is no "documented gap" arm left: it works or it is a finding.

Note T1-8's constraint applies to the borrower: `agent pty` reads the workspace
secret, so the box you run the CLI on must have read access to that node's
workspace.

- **PASS** = the console renders, the session is interactive, and the provider
  answers on the **lent** credential (not a local one — confirm the lender's
  `airlock.log` shows the grant gate deciding for this session).
- **FAIL** = anything that does not reach the provider: a refusal token, a spawn
  failure, a wedge on child exit, or a console that renders but authenticates
  against something other than the lent credential.

**Deliberate-failure half — cheaper and sharper than T2-5's, so do it first.**
Before T2-4b, the same command is refused **immediately**: the work admission is
the first thing `serve_create` asks, ahead of the credential lookup and every
host-capability question.

**Pass:** the CLI prints, verbatim,

```
FATAL: host refused: work_not_admitted: this node does not run work for that account — its operator admits one with `ducktape node work admit <account>`
```

in under a second, with **no attempt burned and no timeout**. The borrower's
`daemon.log` carries one `WARN`, target `ducktape::term`,
`reason="work_not_admitted"`, `node=<8 hex>`, message
`peer session create refused` — the peer's NODE key prefix, **never an
account** (pinned by a behavioural test that asserts the detail does not echo
it).
**Fail:** a hang (the refusal is not upstream of the credential read), or any
refusal naming the account.

---

## 4. On/off isolation matrix (I) — 6 steps

**The claim:** separate processes mean separate failure domains. With all three
enabled, toggle each independently and prove the others are unaffected.

### I-0 — state what `disable` does and does not revoke
**This is an assertion about truth, not a wish. #829 changed half of it.**

`disable` removes the grant from `services.toml`, submits the shrunk announce
set as a transaction, prints the retired id and the retraction height, and
**that is all**:

| `ducktape service disable compute` | |
|---|---|
| removes the `[[service]]` row from `services.toml` | ✅ immediately, **before the submit** |
| retires the instance id (a re-enable mints a **fresh** one) | ✅ |
| retracts the announce **on chain**, reporting a height | ✅ **[#829] synchronously**, not "on the next announce tick" |
| stops the daemon process | ❌ never |
| tears the ws link down | ❌ never |
| kills running containers | ❌ never |
| cancels in-flight work | ❌ never — the daemon read its grant once, at its own boot |

The CLI says so itself:
`stop the daemon too: a running `service run compute` keeps serving what it
already holds`. **Assert that sentence, not a revocation.**

**[#829] `disable` now needs a reachable submitting node — and fails partially.**
With no http surface it refuses outright: *"Revoking consent is a transaction
now, so it needs a reachable node and a finalizing chain — a grant cannot be
revoked while the node is down."* But there is **no `node_identity` precheck**,
so with the node merely *down* the verb still **destroys the local grant first**
and only then fails at the submit, with
`compute's grant is revoked but the announce was NOT retracted — this node
retries every 10s until it lands: <inner>`.

**Deliberate-failure half, and it is a real finding to record.** Stop the node,
run `service disable compute`, and assert: nonzero exit, empty stdout, **and
`services.toml` no longer carries the row**. The grant is gone while the chain
still announces it. The liveness watcher reconciles when the node returns —
verify that it does, by restarting the node and polling the registry until the
tag disappears. If it does not reconcile, that is the finding.

### I-1 — disable compute while an agent pty is live
Start a pty session (T1-8). With it live, `service disable compute`.
**Pass:** the pty session **keeps running** — proved by typing into it and
seeing the echo *after* the disable returned, not by the absence of a
disconnect; and `service list` shows compute gone from the grants and agent
still `✓ enabled`.
**Fail:** the pty drops, or input stops echoing.

### I-2 — disable agent while a compute run is in flight
Submit a long `agent sched` run, then `service disable agent`.
**Pass:** the run completes and **delivers its result into `recent_runs`** —
read the outcome, do not infer it from the absence of an error.

### I-3 — disable airlock while a lent-credential run is in flight
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
30 s and look once**. Re-read the container list on **both** sockets and record
both.
**Known residual to assert honestly:** a SIGKILLed *airlock* daemon leaves its
route in `gateway-routes.json` forever — nothing re-validates the port and there
is no eviction. The borrower is not misled (connect-refused → 502 →
`airlock_gateway_unreachable`), so what is missing is the eviction, not the
diagnosis. This is a documented gap; record it, do not file it as new.

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
With compute and agent both serving and both busy:

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
**This step could not fail as written. The fix is P-4b's.**

Make the image store genuinely empty (do this **with the daemon stopped** —
`PodmanService::claim` treats two supervisors on one root as the hazard it
exists to prevent). **`rm -rf` does not work here**: this is a rootless overlay
graph root, UID-mapped into a user namespace, so `rm -rf` exits 0 having removed
the small readable fraction — leaving the images in place, so the "cold" run is
warm, no pull happens, and the step passes having tested nothing.

```bash
stop_by_config "service run compute"
podman unshare rm -rf "$WS/storage/services/compute/podman/storage"
```

**Prove the store is actually empty before starting** — this assertion is the
step:
```bash
du -sh "$WS/storage/services/compute/podman/storage" 2>/dev/null   # gone, or ~0
```
Restart the daemon, then:
```bash
curl -s --unix-socket "$CSOCK" http://d/v5.0.0/libpod/images/json   # MUST be []
```
**If the image list is non-empty, the reap did not work and K-1 has not
started.** Then submit one run (T1-7).

**Expected behaviour, stated up front:**
1. `create` POSTs `/containers/create`, gets **404 image not known**
2. it calls `POST /images/pull?reference=<image>` and retries `create`
3. the run proceeds normally

**Observable / pass:**
- the run completes, and the image is now present in **that daemon's** store
  (the same `images/json` query, now non-empty)
- the *agent* daemon's store is **still empty** — one image store per service is
  the accepted cost, and this proves it
- **no** `worker_error` in `compute.log`

**Fail:** the run fails at create.

> **The pull is completely invisible.** `crates/services/sandbox/src/podman_api.rs`
> contains **zero `tracing::` calls**, so there is no "pull started"/"pull
> finished" line and no duration. A cold first run looks exactly like a hang.
> Budget several minutes for `node:22-slim` (~230-250 MiB) and say so in the
> report; do not treat the silence as a wedge.

> **A trap in the pull path itself:** libpod's pull endpoint returns **HTTP 200
> on a *failed* pull** — the verdict is an `{"error":…}` line inside the
> streamed body (`pull_failure`, `podman_api.rs`). If a run fails right
> after a cold start with no obvious cause, that is where to look.

### K-2 — the claimed residual: does a cold winner lose its lease to its own download?

The claimed residual is that the pull happens at first run *inside* the lease
window, so a cold winner can lose its lease to its own image download.

**The arithmetic does not support that story, and this step exists to settle
it.** An agent run's lease is `RUN_LEASE_VIEWS = 1024` views
(`crates/modules/apps/runs/src/lib.rs`) at `BLOCK_TIME = 1 s` ≈ **17
minutes**; the host heartbeat fires every **10 s** (`compute/pool.rs`) and
is `select`ed against the run future, so it covers the create/pull; and each
renewal past the half-window resets expiry to `height + 1024`. No path was found
that makes a `node:22-slim` pull outlast that.

> Do not confuse it with `JOB_RUN_LEASE_VIEWS = 1000` in the same file — a
> different constant on the jobs lane.

**So the expected result is: the cold run completes and the lease is never
lost.**

**How to tell a pass from a failure:**
- **PASS:** run completes; `grep -c 'lease_renew_failed' "$LOGS/compute.log"` = 0;
  the saga shows **one** attempt.
- **RESIDUAL CONFIRMED (report loudly):** `recent_runs` shows the run on
  `attempt: 1` or higher, **or** `lease_renew_failed` appears. Then capture
  `RUST_LOG=ducktape::saga=debug` output — the cause is either the `RenewLease`
  origin check refusing or the renew submit failing.
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
**[#829] Both halves now also report a height.** Record them; a re-enable that
returns the same id *or* the same height would break the epoch property.

### R-2 — node restart, daemons still up

Restart the node with the daemons left running.
**Observable / pass:**
- `admin.token` and `service-link.token` are **both freshly minted** (contents
  differ from before — compare hashes, never print them)
- **[#827] a ws subscriber to a gated topic must re-present the NEW secret.** An
  old `run-output:` subscription's token no longer matches; a resubscribe with
  the stale value gets `reason="topic_not_admitted"`. Assert that, then assert a
  resubscribe with the fresh file succeeds.
- the **agent daemon's ws link is refused** until it re-reads the new
  `service-link.token`, then reconnects. Expected transient in `agent.log`:
  `reason="link_refused"` … `refused: present this node's service-link token…`,
  followed by a successful redial. **A permanent wedge here is a FAIL.**
- the compute daemon rides through: its heartbeat logs
  `reason="hello_failed" attempts=1` (then every 30th) while the node is down and
  `"signal restored"` after
- a daemon started *while the node is down* exits loudly with
  `could not read this node's identity: …` and does **not** spin. Assert the
  process is gone, not looping.
- **[#829] the announce watcher's 30 s settle.** After the restart the registry
  may legitimately lag by `SETTLE = 30 s` before the watcher submits anything.
  Poll for the tag's return; do not read the first empty query as a retraction.

### R-3 — deliberate build skew must WARN, not refuse

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

**Two more behaviour changes to confirm while you are here:**
- `service run` **does not refuse to start on a git-absent build**. It used to
  exit `this binary has no build identity; rebuild it from a git checkout`; it
  now signals `build = "unknown"` and serves. Test it by building from a tarball
  or with `.git` hidden.
- the `enabled-but-absent` hint does not mention builds. It is
  `enabled but not signaling — is its daemon running (ducktape service run), and
  pointed at this node's http surface?` — **if you see the old text naming
  `reason build_mismatch`, the binary is not from P-2.**

**Fail:** HTTP **409 `build_mismatch`** on the hello → the binary is not from
P-2; both build refusals were deleted in this tree.

> **The one skew that is still hard, and it is NOT the build stamp.** #820
> deleted `ServiceAttach.build` **and** put `deny_unknown_fields` on `ClientMsg`.
> So a **pre-#820** daemon (which still sends `"build"`) attaching to a
> **post-#820** node is refused with a `BadFrame` naming the field. That is
> correct and intended — but it means "skew warns instead of refusing" is true
> *within* post-#820 builds and false *across* the #820 boundary. Say which you
> tested. On this tree both sides are post-#820, so the only way to see it is to
> deliberately build an old binary.

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

> **X-1a/X-1b are, with T2-5 and T2-6, the steps that would give the cross-box
> terminal lane its first live two-box run** (§3). Weight them accordingly in the
> report, and do not let either half pass on an absence.

### X-1 — agent on A, compute only on B

> **This step could not fail before it was amended, and that is exactly the
> hazard P-9 exists to catch.** It submits an *unpinned* run and waits for the
> borrower to claim it. Under work admission's default the borrower will not bid
> for a stranger's announcement — and an unpinned saga with no `deadline` can
> never be cranked out of `Pending` (assumption-audit A9), so the symptom is a
> **silent park**: no error, no timeout, nothing in `recent_runs`, forever. Run
> the negative FIRST so the difference is observable, and never read "it is
> still going" as slow.

**X-1a — the negative, before `node work admit`.** Disable compute on the dev
box; leave it enabled and serving on the borrower. Submit an unpinned run from
the dev box.
**Pass:** the borrower's `compute.log` carries exactly one WARN, target
`ducktape::saga`, `reason="work_not_admitted"`, message **`compute claim
refused`** (the claim lane's message, distinct from T2-5's `compute attempt
refused`) for that saga — once, because the decision is latched — the saga stays
`Pending` with `assignee: null`, and **no container is created**:
```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"saga","query":{"get":{"saga_id":"'"$SAGA"'"}}}'
curl -s --unix-socket "$CSOCK" 'http://d/v5.0.0/libpod/containers/json?all=true'   # unchanged
```
**Fail:** the run executes anyway (the claim lane is ungated), or the saga parks
with **no** warn — a silent refusal is the one outcome this amendment exists to
forbid.

**X-1b — then admit and re-run.** On the borrower,
`"$D" node work admit <dev-box-account-hex> -n "$CHAIN"` (T2-4b), then submit a
**fresh** unpinned run **with no restart of anything**.

> **The word "fresh" is load-bearing, and skipping it looks exactly like a
> broken admission.** On the **claim lane the refusal is latched, not just its
> log**: `ClaimState::NotAdmitted` is inserted against the announcement, and the
> code says why in its own `ponytail:` note — *"the DECISION is latched, not just
> its log — so admitting the submitter later does not re-open an announcement
> already seen."* The latch clears only when the announcement leaves the
> projection. **Re-polling X-1a's saga after admitting will never pick it up**,
> and an hour spent concluding "admission does not work with no restart" is the
> predictable outcome. Submit a new run. (This is a deliberate shortcut with a
> named ceiling, not a defect — but it is invisible from the outside, which is
> why it is written here.)
**Pass:** the run executes on the borrower and its result commits. Proof the
placement was real, not local:
```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":{"capable_providers":{"capability":"compute","demands":{"cores":1}}}}'
```
returns only the borrower's key, and the borrower's `compute.log` — not the dev
box's — carries the run, **and a container labelled
`io.ducktape.managed=compute#…` appeared on the borrower's compute socket.**
That last clause is what makes "it ran over there" an observation instead of an
inference.

### X-2 — the kind tag is in the committed registry
```bash
curl -s http://127.0.0.1:9971/v1/query -H 'content-type: application/json' \
  -d '{"target":"capability","query":"all"}'
```
**Pass:** each node's announced set is **sorted** and contains its granted kinds
(`compute`, `agent`, `airlock`) *as tags*, plus the executor-tag intersection of
`grant.capabilities ∩ hello.capabilities` **per kind**. A daemon's hello can
never vouch for another kind's tag.

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

> **[#829] where this is enforced moved.** `Services::validate` runs the
> announce-set computation on every `load`, so an illegal tag baked into
> `services.toml` now fails the **node boot** rather than being dropped at
> announce time. Both paths exist: a *daemon* offering an illegal tag is
> dropped-and-warned; a *grant file* carrying one is fatal. Say which you
> triggered.

### X-3 — a rejected announce reports, then re-arms

Force announce rejections (e.g. submit while the node cannot apply).
**Pass:** the warn appears at attempts **1, 32, 64, …** — never on every tick:
```
WARN … reason="announce_failed" attempts=<n>
```
> **You cannot directly observe the 32-block re-arm.** The submit-failure path
> has **zero `tracing` calls** of its own. The only signal is that a later tick
> emits a fresh success marker `"capabilities announced"` with a `height` — so
> run `ducktape::modules=debug` or you will see nothing. Also:
> `ANNOUNCE_RETRY_BLOCKS = 32` and `REJECTION_REPORT_EVERY = 32` are two
> different constants that share the value 32; a "32" in a log identifies
> neither.

---

## 9. Invariants to assert at the end (V) — 5 steps

### V-1 — consensus root hash unchanged by any of it
**[#830] this is a TEST now, not an eyeball.**

The production genesis root hash is **pinned in the tree**:
`GENESIS_ROOT_HASH` in `bin/node/src/host_state.rs`, currently
`e1c23f187ef9aa6880f7140e1ce940205ff4d65803c00f6a0b0cda69897ecc35`, asserted by
`production_genesis_root_hash_is_pinned`. `seeded_lifecycle` commits
`sha256(component.wasm)` per wasm tenant into lifecycle's MerkleStore, so **any
rebuilt guest moves this hash** — which is precisely the event the test exists
to make loud.

```bash
cargo test -p node-bin --bin ducktape production_genesis_root_hash_is_pinned
cargo test -p node-bin --bin ducktape genesis_registry_matches_module_ids
make wasm-modules-check
```

**Pass:** all three green.
**Fail:** any difference → a service change reached consensus, which it must not.

> **The test does not replace the cross-box comparison, and this is the one part
> of V-1 that stays manual.** The pin proves *this tree's* composed genesis is
> unmoved. It says nothing about whether box A and box B agree. Keep that half:
> ```bash
> grep 'genesis root_hash=' "$WS/daemon.log" | tail -1     # on BOTH boxes
> "$D" node status -n "$CHAIN"                             # on BOTH boxes
> ```
> and assert the two are byte-identical. Drop the old "compare against T1-1"
> half — the test covers it and does so without a human transcribing 64 hex
> characters.

> Do not confuse it with the sim pin. `DEFAULT_GENESIS_ROOT_HASH` in
> `bin/simnode/tests/topology_set.rs` is a **different** hash over a
> **different** 14-module set (it excludes `capability`, `hello`, `governance`,
> `lifecycle`) and its own doc disclaims consensus relevance. Running
> `cargo test -p simnode --test topology_set` is a fine sanity check and is
> **not** the consensus pin.

### V-2 — `/v1` additive only
**As written this could not fail. Re-anchored.**

The old form was `git diff origin/dev...HEAD -- bin/noded/src/lib.rs | grep '^-.*\.route('`.
On this tree that diff is **empty by construction** — the pass runs *on* dev, so
the grep returns 0 no matter what the route table says. It was a green light
wired to nothing.

Assert the route table itself instead:

```bash
# at P-2, once:
routes() { grep -oE '\.route\("(/v1[^"]*)"' bin/noded/src/lib.rs | sed 's/.*("//' | sort; }
routes > "$LOGS/routes.baseline"        # NOT /tmp — §0.3
wc -l < "$LOGS/routes.baseline"         # 33 on this tree; record it

# at V-2, at the end of the pass:
routes > "$LOGS/routes.now"
diff "$LOGS/routes.baseline" "$LOGS/routes.now"
```

**Pass:** `diff` is empty, **or** contains only `>` lines (additions). New routes
are fine; a changed or removed one is a wire break.
**Fail:** any `<` line — a route present at P-2 and gone at V-2.

This also gives the report a concrete number — the `/v1` surface is 33 routes on
this tree — instead of an unfalsifiable "additive only".

### V-3 — no credential name or token in any log
**Assert the logs exist first.** The old form was three greps expecting empty
output, which a missing or truncated log file satisfies perfectly.

```bash
for f in "$WS/daemon.log" "$LOGS"/*.log; do
  test -s "$f" || { echo "EMPTY OR MISSING: $f" >&2; exit 1; }
done
wc -l "$WS/daemon.log" "$LOGS"/*.log     # record; a suspiciously short file is a finding
```

Then:
```bash
grep -rniE 'admin\.token|service-link|sk-|Bearer |accessToken|refreshToken' \
     "$WS/daemon.log" "$LOGS"/*.log | grep -v 'admin.token in the node' | head
```
**Pass:** no secret **values**. A credential **name** legitimately appears
(`credential=<name>` on the airlock grant gate) — that is by design; the
**account** never does, and neither does any token.

Also assert the doctrine directly: **no URI path or query string is logged** —
`/.duck/ws/{token}` carries a capability in the path.
```bash
grep -nE 'path=|uri=|/\.duck/ws/' "$WS/daemon.log" "$LOGS"/*.log | head   # expect empty
```

**And check the newest surface:** #827's topic refusals are `&'static str` by
construction precisely so a secret cannot reach the formatter. Confirm no
`topic_not_admitted` line carries anything but the reason token.

### V-4 — the admin gate actually refuses
Run against a node **before** T1-6 (no owner yet) so the operator-token arm is
the one under test:

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
```bash
for p in $(pgrep -u "$USER" -x ducktape); do
  tr '\0' ' ' < "/proc/$p/cmdline" | grep -q 'service run' || continue
  # a daemon must never have identity.key open
  ls -l "/proc/$p/fd" 2>/dev/null | grep -c 'identity.key'
done
```
**Pass:** `0` for every daemon process, and the **node** process is the one that
answers for the identity. Structural proof: `ServiceConfig` has no field a secret
could live in, `resolve_service` never opens `identity.key`, containment runs
one way (`Resolved` holds a `ServiceConfig`, never the reverse), and there is a
source-parsing lint test on it (`the_daemon_path_cannot_name_the_node_key`).

**Also assert the honest caveat**, and put it in the report next to §0.4's:
`/v1/status` carries no auth, so a same-uid process that binds `http_listen`
first could answer with any `public_key`. **That is not a regression and not a
blocker** — a same-uid attacker can read `identity.key` directly anyway. The
boundary this pass proves is *between processes of one user*, not against that
user. It becomes load-bearing only when a daemon has no workspace.

---

## 10. Failure triage table

Every expected failure mode, and the one string that identifies it. All reason
tokens are stable snake_case and greppable.

### Borrower side — broker (`crates/services/broker/src/lib.rs`, WARN, target `ducktape::gateway`, message `airlock session not opened: …`)

The classifier reads the gateway's **own body token first**, because three
different refusals wear HTTP 403; the status table is only the fallback.
Pinned by `every_refusal_carries_the_name_of_what_actually_failed`.

| `reason` | what actually happened |
|---|---|
| `airlock_gateway_unreachable` | transport error with no HTTP status, **or** HTTP 502. The lender's daemon is down, or its node cannot reach the daemon's loopback port. **Includes the stale-route case** (route registered, nothing listening). |
| `airlock_route_or_credential_absent` | HTTP 404. No `airlock` route published on the lender, or no credential by that name in its store. |
| `credential_not_granted` | HTTP 403 with that body token. The lender's grant gate consulted its committed record and the **vouched-for** account is neither owner nor grantee. **[#833] this no longer fires for a missing account** — see below. |
| **`airlock_caller_account_unverified`** | **[#833] NEW.** HTTP 403 `caller_account_unverified`. The request reached the lender's listener **without the node's gateway proxy vouching for a caller** — so there was no verified identity to check a grant against. Distinct from "denied". |
| `airlock_grant_authority_unavailable` | HTTP 503. The lender's gate could **not ask** its authority (query timeout, node restarting, resident not yet serving). Never means "denied". |
| `airlock_gateway_refused` | any other HTTP status. |
| `airlock_gateway_malformed_response` | body not JSON / wrong shape / truncated / non-base64 token. |
| `gateway_seal_pk_mismatch` | well-formed response whose sealed token will not open under the pinned key — the gateway's real seal key ≠ the published one. **Historically this masked every pinned-path failure**; the taxonomy is what fixed that, so seeing it now means what it says. |
| `airlock_session_unclassified` | **a code defect, not an ops condition.** Unreachable by construction; it exists so a new failure path that forgets to tag itself does not borrow another arm's name. |

> **[#833] the retired behaviour — delete it from your mental model.** The old
> runbook said `credential_not_granted` "also fires when the request carried
> no/undecodable `account_b64` at all — a 403 where no grant was ever
> consulted." **That path is gone.** `SessionRequest` now has exactly three
> fields (`sub`, `client_eph_pk_b64`, `body_seal`) and is `deny_unknown_fields`,
> so a request that names an account is **422 Unprocessable Entity** at the
> decode boundary, before any gate runs
> (`a_session_request_cannot_name_an_account_at_all`). An unvouched caller is
> **403 `caller_account_unverified`** (`a_session_no_proxy_vouched_for_is_refused`).
> A malformed request is **422**, never a grant refusal
> (`a_malformed_session_request_is_a_decode_error_never_a_grant_refusal`).
> Three outcomes that used to collapse into one now have three names.

### Lender side — the grant gate

Two layers, and they are not the same list. Do not conflate them.

**Node-side decision** (`bin/node/src/airlock.rs`, DEBUG, target
`ducktape::gateway`, carries `credential=<name>`, never the account):

| `reason` | trigger | resolves to |
|---|---|---|
| `credential_record_absent` | query OK, gateway module returned no record for that name | `Refused` |
| `grant_authority_unavailable` | the `/v1/query` itself failed — **note: no `airlock_` prefix on this side** | `Undetermined` |
| `credential_not_granted` | record exists; the vouched account is neither owner nor grantee | `Refused` |

**Gateway-side HTTP** (`crates/airlock/src/server.rs`) — the plain body tokens a
borrower actually sees:

| body | status |
|---|---|
| `credential_not_found` | **404** |
| `caller_account_unverified` | **403** |
| `credential_not_granted` | **403** |
| `grant_authority_unavailable` | **503** |
| (decode failure — unknown/missing field) | **422** |

`credential_record_absent` is a **node-side log token only**; on the wire both
`Refused` arms collapse into `403 credential_not_granted`. A report that lists it
as an HTTP reason is reading the wrong layer.

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
| daemon exits at boot, node down | `could not read this node's identity: the node is not running` |
| daemon exits at boot, node up but unpublished | `this node published a 0-byte mesh identity, not 32 — build mismatch` — **the message is wrong** (T1-2), the intended one is unreachable |
| hello refused | HTTP 400 `malformed_hello` / 503 `catalog_full`. A 409 `build_mismatch` or 503 `build_identity_unavailable` means the binary is not from P-2. |
| daemon and node on different builds | WARN `reason="build_skew"` once per transition — **serves anyway** |
| agent ws link refused, token/holder | `reason="link_refused"`, detail `refused: present this node's service-link token, and only one agent service may attach` |
| **mixed binaries: a pre-#820 daemon against this node** | ERROR `reason="link_refused"` whose `detail` is a serde message `unknown field \`build\`, expected \`kind\` or \`token\``. **Silent on the node side** and it reconnects forever. Only reachable by deliberately building an old binary. |
| a `Create`/`Command` frame the daemon cannot decode | WARN `reason="malformed_command"` — **serde text discarded, link stays up, the node's session create hangs with no timeout** |
| daemon cannot reach the node | WARN `reason="hello_failed" attempts=N` (1, then every 30th); recovery = INFO `"signal restored"` |
| two daemons, one graph root | `another service daemon (pid <N>) already owns <socket> — stop it before starting this one` |
| podman service never came up | `podman service did not answer on <path> within 5s` |
| flat `sandbox =` key | `FATAL: "<path>": … invalid type: string "…", expected struct SandboxToml` |
| `[sandbox]` present, compute ungranted | WARN `reason="compute_not_granted"` |
| **[#829] enable/disable submit failed** | `… is granted but NOT announced …` / `… grant is revoked but the announce was NOT retracted … this node retries every 10s until it lands: <inner>` |
| **[#829] settle expired** | `400 {"error":"timed out awaiting finalization — re-query on the next block"}` (validator) / `"… timed out awaiting the relay answer - re-query on the next block"` (resident — note the ASCII hyphen) |
| **[#829] auto-enable declined, daemon still serving** | `reason="enable_not_announced"` |

### ws stream (`bin/noded/src/stream.rs`, DEBUG, target `ducktape::stream`)

| `reason` | code | meaning |
|---|---|---|
| `topic_not_admitted` | `forbidden` | **[#827]** a workspace-gated family (`run-output:<id>`, `term:<session>`, `term-cmd:<session>`) with no/wrong `token` in the subscribe frame. **A `subscribed` frame still follows, with the topic absent from the map** — the error frame is the only signal. |
| `unknown_topic` | `unknown_topic` | no family owns that name |
| `unknown_module` | `unknown_topic` | real family, a module this node does not index — a **separate** reason on purpose |
| `malformed_run_id` | — | a `run-output:` id that is not 64 hex chars. Was silently eating **every** `agent sched` run's live output before `fresh_dispatch_id` went 16→32 bytes. |

### Work admission (`bin/node/src/work_admission.rs`)

| `reason` | where | meaning |
|---|---|---|
| `work_not_admitted` | WARN `ducktape::saga` `"compute attempt refused"` (lease lane) / `"compute claim refused"` (claim lane) / WARN `ducktape::term` `"peer session create refused"` (pty lane) | the policy does not admit that account. Once per attempt / latched per announcement. |
| `work_caller_unbound` | same | the caller could not be bound to an account at all |
| `work_policy_unreadable` | WARN `ducktape::service` `"work admission cannot read its policy"`, carries `%error` | `work-admit.toml` present but unparseable — **fails closed** |
| `work_authority_unavailable` | — | a three-state verdict, **not** a refusal. Never read it as "denied". |

### Shared pty (`bin/noded/src/term_consensus.rs`, target `ducktape::term`)

Not exercised by any step here — see §0.4b.

| `reason` | level | meaning |
|---|---|---|
| `command_not_channel_owner` | DEBUG, `"term_command_refused"` | **[#835]** a shared-pty command post whose verified author is not `Channel.owner`. The post does **not** reach the pty. `debug` on purpose: it can fire per post. |
| `command_deleted` | DEBUG | tombstoned message |
| `channel_unreadable` / `channel_unowned` | WARN, `"term_consensus_projector_refused"` | startup fail-closed; the projector does not start |

### Placement / announce

| symptom | identifier |
|---|---|
| illegal capability tag dropped | WARN `reason="announce_tag_illegal" dropped=<n>` (latched) |
| illegal tag in `services.toml` | **fatal at node boot** — `Services::validate` runs the announce-set computation on every load |
| too many tags | WARN `reason="announce_over_cap" dropped=<n> cap=64` (latched) |
| announce submit failed | WARN `reason="announce_failed" attempts=<1,32,64,…>` |
| announce landed | INFO `"capabilities announced" height=<N>` — the marker to wait on |
| grant file unreadable | WARN `reason="grant_unreadable"` |
| run not placed | **no log line** — query `capability` `capable_providers` and `saga` |
| work refused by admission | see the work-admission table above |
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

## 11. Existing QA recipes: what today's tree does to them

- **`skills/qa/SKILL.md` — current and correct; no edit needed.** It carries the
  `x-ducktape-admin-token` curl and the "never paste that token" line. It also
  now states the `app/` fact directly: *"`app/` itself was rewritten in place,
  not removed"* — `ducktape-app` is a live workspace member (`Cargo.toml`, and
  its own `#[cfg(test)]` suites live in `app/src/main.rs` and
  `app/src/backend.rs`), and the skill's run list includes
  `cargo test -p ducktape-app` with the note that it *"is not optional and not
  covered by the node lanes above"*. **Run it in this pass.**
  One thing the skill still does not say: on a node with a committed owner the
  admin token is refused and an **owner PoP** is required (P-8). Worth a
  follow-up line.
- **The "app/ was deleted" drift is fixed.** Four docs used to claim it; #832
  swept them (`README.md`, `ops/README.md`, `skills/qa`, `skills/sim-lane`, plus
  comments in `bin/node/src/util.rs` and `bin/noded/src/log.rs`). Re-grepped at
  `fc6334d8f`: **zero surviving claims.** Do not re-introduce one.
- **`app/admin-client.ts` does not exist**, and never did at that path — the
  design that named it
  (`docs/superpowers/plans/2026-07-14-w2-owner-control.md`) specified
  `app/src/domain/admin-client.ts`. Neither exists; the Tauri/TS shell was
  replaced by the Iced rewrite. `docs/superpowers/specs/2026-07-14-w2-owner-control-design.md`
  still describes the loopback-trust admin model — documentation drift, not a
  finding for this pass.
- **`ops/demo-clear.sh`** sends the admin token and falls through to its pid
  sweep otherwise. See P-8 for why its admin call will 403 on an owned node.
- **`ops/worktree-clean.sh` — untouched and unaffected.** No `curl`, no `/v1/`;
  it reaps by pidfile + `/proc/<pid>/exe` + `--config` identity, never
  `pkill -f`. **It does not know about rootless podman storage** — a worktree it
  removes may leave a UID-mapped graph root behind (P-4b). Reap storage first.
- **`ops/agent-system` — unaffected.** Only `/v1/query` and `/v1/submit`. Still
  the fastest way to read `runs pending_runs` / `recent_runs` and
  `capability all`.
- **`ops/demo-seed.sh`, `demo-app.sh`, `demo-gateway.mjs`, `demo-kanban.mjs`,
  `dogfood-forge.sh` — unaffected.**
- **`CLAUDE.md`'s `/v1/log-filter` recipe — still correct.** That route is not
  gated.
- **`Makefile`** references a stale `/v1/shutdown` in a comment (the route moved
  to `/v1/admin/shutdown` long ago). Cosmetic, pre-existing.
- **`docs/superpowers/plans/2026-07-26-wave3-scope-enforcement.md` is now wrong
  in two places**, and both should be corrected before anyone builds on it:
  1. it documents the service-link authentication as "kind == agent, **build
     equality**, node-wide link token, single holder" — #820 deleted the build
     check from `take_service_link` entirely;
  2. its topic table says **"7 families, none gated"** — #827 gated three of them
     behind the workspace secret. Wave 3's premise survives; the inventory does
     not.
- **`docs/superpowers/plans/2026-07-26-work-admission.md` has one stale
  paragraph** (§P10): it describes the shared-pty fix as a `MembersOnly` post
  policy with a participants roster. #835 **rejected** that and gated at
  `project_message` instead (§0.4b). Its §4 on delegation is current and is the
  reference for §0.4a.
- **`bin/node/tests/remote_session.rs`'s own header comment is stale** — it says
  the test *"SKIPS loudly when podman is absent"*; since #831 it FAILS. (That
  file is also the known-red baseline below; the two are unrelated.)
- **`ops/completions/ducktape.{bash,zsh}`** were updated by #832 and now carry
  `--host-node`. If a completion offers `agent --node <name>`, the completions
  are stale, not the CLI.

**Test-suite hazards this pass no longer inherits (§P-9):** `dispatch_e2e`,
`dogfood_loop_e2e` and `sched_pinned_run` all route their host-capability gate
through `nettest::skip_without` now. They **fail** on an under-provisioned box
instead of passing green.

**Known-red baseline:** `bin/node/tests/remote_session.rs` is pre-existing red
(§0.3). Record it; do not fix it here.

---

## 12. Report template

For each step: `PASS` / `FAIL(<reason token or log line>)` /
`EXPECTED-REFUSAL(<step>)` / `BLOCKED(<what is awaited>)`. Never blank.
**`SKIPPED(reason, PR)` is retired** — there are no open PRs to skip for.

Plus, at the top:
- the pass's SHA from P-2 and whether the tree was clean
- the `/v1` route count from V-2, and the baseline diff
- both boxes' genesis root hash (must match each other), and the result of
  `production_genesis_root_hash_is_pinned`
- the pre-publish window measured in T1-1
- `df -h /tmp` before and after, and the P-4b storage figure
- every step that was BLOCKED or EXPECTED-REFUSAL, and why
- **the honest list of what this did not prove** — copy §0.4 verbatim, including:
  - `/v1` is trusted-local; `origin_guard` passes every `Origin`-less caller
  - `grant.scopes` gates nothing
  - a same-uid process can read `identity.key`
  - the `logs` and `module:<id>` ws topics are Public
  - **delegation is not implemented** (§0.4a) — the refusal in T2-4's negative
    half is correct behaviour, not a defect
  - the continuation-lane deletion (#839) is **in** this tree but **untested by
    this pass** — a green run is not evidence the consensus-takeover class is
    closed

---

## 13. Step count

| section | steps |
|---|---|
| 1. Preconditions (P) | 9 |
| 2. Tier 1 (T1) | 8 |
| 3. Tier 2 (T2) | 7 (incl. T2-4b) |
| 4. On/off isolation (I) | 6 |
| 5. Podman co-tenancy (C) | 3 |
| 6. Cold start (K) | 2 |
| 7. Restart and skew (R) | 4 |
| 8. Cross-node placement (X) | 3 (X-1 has two halves) |
| 9. Invariants (V) | 5 |
| **total** | **47** |

Twelve of those carry a **deliberate-failure half** that must be run first:
T1-3, T1-7, T1-8, T2-4, T2-5, T2-6, I-0, K-1, R-3, X-1a, X-2, and P-9's
`DUCKTAPE_ALLOW_MISSING_TOOLS` check. A pass that skipped the negatives has not
established that any of the positives can fail.
