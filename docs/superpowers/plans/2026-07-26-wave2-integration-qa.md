# Wave 2 integration QA — the one terminal pass

- **Date:** 2026-07-26. **Revised:** 2026-07-27 against `dev` @ `60d86b8ec`.
  **Repaired:** 2026-07-27 against `dev` @ `feec0a6db`, from the findings of its
  own first execution. **Re-repaired:** 2026-07-27 against `dev` @ `02ad78b5e`,
  because §0.4a documented delegation as unimplemented **after it shipped** —
  a tester following it would have filed a working, live-verified cross-box
  feature as a defect (§0.4a, T2-4, T2-5, §12).
- **Status:** **partially executed.** The five credential-free sections
  (§Preconditions, §Isolation, §Podman co-tenancy, §Restart-and-skew,
  §Invariants) were run against real nodes, daemons and containers on
  2026-07-27. **They found more defects in the runbook than in the product** —
  eight steps that could not fail, five that could not pass, seven stale
  premises. Every one of those is corrected below and labelled where it sits.
  **Tier 1, Tier 2 and cross-node placement have not been run.** They are what
  this repair exists to make worth running.
- **What the first execution changed structurally:** §0.1 grew from three rules
  to six (rules 4–6 are hazards that pass hit); §0.5 is new (four known-open
  product bugs, being fixed separately — do not re-file them); Tier 1 is no
  longer "no airlock at runtime", because that claim does not survive the code
  (T1-7); and §Isolation is no longer credential-free (§4's head note).
- **Turns into an executable procedure:** the "Integration QA — one terminal
  pass" section of `2026-07-25-service-daemons.md`.
- **Predecessors:** `2026-07-25-services-extraction.md` (wave 1),
  `2026-07-25-service-daemons.md` (wave 2),
  `2026-07-26-assumption-audit.md`, `2026-07-26-work-admission.md`.
- **Target tree:** `dev` @ **`02ad78b5e`** (PR #847 merged), as it stands. **No
  integration branch, no cherry-picks, no open PRs to wait for.** Code facts
  below were established at `fc6334d8f`, re-checked against `60d86b8ec`, and the
  credential-plane ones re-checked again against `02ad78b5e`; `GENESIS_ROOT_HASH`
  is unmoved by every merge since (#843, #845, #846 and #847 each state
  `crates/modules/` is untouched).
- **Nine PRs merged between the last repair and this one** (`60783b146` →
  `02ad78b5e`). Six of them move a step: **#841** (a stopped daemon takes its
  sandbox with it), **#843** + **#847** (delegation, and the lender's record of
  it), **#844** (the lent claude TUI), **#845** (per-run config home) and
  **#846** (CLI ergonomics). §0.2's table carries the rows; §0.4a carries the
  one that inverted an instruction.

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

### 0.1 The six rules this runbook never breaks

**Rules 4–6 were added after the first execution of §Preconditions, §Isolation,
§Podman co-tenancy, §Restart-and-skew and §Invariants.** Every one of them is a
hazard that pass actually hit, and rule 4's quiet case silently passed a step
against a stranger's containers.

1. **Never `pkill -f`, and never `pgrep -f` in a pass/fail expression.** A
   pattern match has already killed an agent's own shell in this repo, and it
   also **matches the invoking shell**: `pgrep -u "$USER" -f -c 'podman.*system
   service'` returned **2** on a box with zero real podman processes, because
   both hits were the shell running the command. That form sat inside P-4's own
   pass block for a full revision. `pgrep -f` is acceptable as a *rough* count
   in prose; it is never the thing a step asserts on. Assert with `pgrep -x
   <exe>` plus an identity check (`/proc/<pid>/exe`, `--config`/`--root`), or
   ask the node to stop itself.
2. **Wait on events, never on durations.** Every wait below names a log line,
   a committed height, a file, or a ws frame. Where a poll loop is used it
   polls **for a state transition**, not for a clock.
3. **A pass is an observed post-state, never the absence of an error.** `rm -rf`
   that exits with an error having deleted 98% of the bytes is the canonical
   failure of this rule (§P-4). If a step's evidence is "no error appeared", it
   is not evidence.
4. **Never glob `$XDG_RUNTIME_DIR/ducktape/*-<kind>.sock`.** The socket name is
   `ducktape-<fnv1a32 of the data dir>-<kind>.sock`, so on a shared box the glob
   returns **every** workspace's socket — three sections of the first pass hit
   this. **The quiet case is the dangerous one:** if your daemon failed to start
   while a stranger's is up, the glob resolves to exactly one path — *theirs* —
   and the step asserts against someone else's containers **and passes**. Same
   class as the port-collision bug that made one test silently drive another
   node (§P-9). Resolve it from your own workspace instead:

   ```bash
   # the ONLY sanctioned way to name a service socket in this runbook.
   own_sock() {  # own_sock <kind>   e.g. own_sock compute
     local root="$WS/storage/services/$1/podman"
     local pid; pid=$(cat "$root/podman.pid" 2>/dev/null) || { echo "no podman.pid under $root" >&2; return 1; }
     readlink "/proc/$pid/exe" 2>/dev/null | grep -q 'podman$' \
       || { echo "pid $pid is not podman — stale $root/podman.pid" >&2; return 1; }
     tr '\0' '\n' < "/proc/$pid/cmdline" | grep -m1 '^unix://' | sed 's|^unix://||'
   }
   CSOCK=$(own_sock compute) || exit 1
   ASOCK=$(own_sock agent)   || exit 1
   ```
   **[#841] the pile-up this rule guards against is now a SIGKILL property, not
   a property of every stop** — a SIGTERM'd daemon takes its podman service and
   its containers with it (§0.5). The rule stands unchanged regardless: a shared
   box still carries other people's sockets, and the quiet case is still the
   dangerous one.

   `<data>/podman/podman.pid` is written by `PodmanService::start`
   (`crates/services/sandbox/src/podman_api.rs`) and the service is spawned as
   `podman --root <data>/podman/storage … system service --time=0
   unix://<socket>`, so the pid file, the `--root` and the socket are one
   verified chain rooted in **your** `$WS`. **A failed `own_sock` is a FAIL of
   the step that needed it — never a reason to fall back to the glob.**
5. **`podman service did not answer on <path> within 5s` is a duration, and it
   fires under load.** `PodmanService::await_socket` polls `_ping` 100 × 50 ms
   and then `FATAL`s + `exit(1)`; it is the one place the product itself breaks
   rule 2, so a tester has to tell a slow box from a defect. Measured on this
   box: **0.125 s warm / 0.163 s cold** when started alone — and **the 5 s
   budget blown** at load average 19.8 with six concurrent services. Before
   recording it as a product defect, re-run the same start **alone** and record
   both numbers plus `uptime`. Timed out under load = record as
   `FLAKE(load=<avg>, alone=<secs>)`; timed out alone on an idle box = a real
   finding. Known-open (§0.5).
6. **A stray environment variable silently changes what you are testing.**
   Print this block into the report before the first step, and never set any of
   them to make something go away:

   ```bash
   for v in ANTHROPIC_API_KEY CLAUDE_CODE_OAUTH_TOKEN DUCKTAPE_CRED_REUSE_ARTIFACT \
            DUCKTAPE_ALLOW_MISSING_TOOLS DUCKTAPE_NODE DUCKTAPE_ADMIN \
            DUCKTAPE_PODMAN_SOCKET OPENAI_API_KEY; do
     printf '%-32s %s\n' "$v" "${!v:+SET}"      # SET/unset only — never the value
   done
   ```
   - **`ANTHROPIC_API_KEY` preempts everything.** `AnthropicAuth::from_host`
     (`crates/services/broker/src/lib.rs`) tries `ANTHROPIC_API_KEY`, then
     `CLAUDE_CODE_OAUTH_TOKEN`, then `~/.claude/.credentials.json`. A stray key
     turns the subscription path into the API-key path with no log line.
   - **`DUCKTAPE_CRED_REUSE_ARTIFACT` validates nothing.** `cred add` does a
     bare `std::fs::copy(&src, dir.join(provider.artifact()))` and then only
     checks the destination **exists** (`bin/node/src/cred_cli.rs`). That is how
     one agent ran the credential-dependent steps of the first pass against a
     **fabricated** credential and got plausible-looking results. Anything it
     produced is unverified — see T1-6's HOLD and T1-7's re-verification note.
   - **`DUCKTAPE_ALLOW_MISSING_TOOLS=1`** is the escape hatch P-9 exists to
     police. Must be unset.
   - **`DUCKTAPE_NODE`** sits on the `--node` address ladder (#832) and will
     silently retarget `user`/`fs`/`agent`.

Helpers used throughout (paste once per shell). Two more are defined where they
are first needed and are used by later sections: **`own_sock`** in rule 4 below
(needs `$WS`, set in T1-1) and **`saga_get`** in T1-7 (the only correct read of
an `agent sched` outcome — see that step's box).

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
# A published identity is a STRICTLY stronger gate than a 200 — and it is still
# NOT "committed state is loaded". See the correction under this block.
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

> **CORRECTION (measured, first pass). The bind-before-publish window this
> runbook was built around is CLOSED on this tree — but `await_published` is
> still the right gate, for a different reason.**
>
> The old claim was that `ducktape node run` answers `/v1/status` with a 200 and
> `public_key: ""` for a window after `boot::surfaces::bind`. **Two boots,
> ~750 polls at 5–10 ms: zero 200s with an empty `public_key`.** On a genesis
> boot: 755 connection-refused polls, and the **first** 200 already carried the
> key. The mechanism: `bind` spawns the app surface on its own OS thread that
> must first build a whole multi-thread tokio runtime
> (`bin/node/src/boot/surfaces.rs`), while `status.publish(NodeStatus { …
> public_key … })` runs inline on the main task a few hundred lines later
> (`bin/node/src/main.rs`). The publish wins by a wide margin, every time.
> **Do not report a false green here as a finding, and do not spend the pass
> trying to observe the window.**
>
> **What is still true, and is the reason the helper stays:** a published status
> does **not** imply committed state is loaded. On a **warm restart** the first
> 200 carried the key with `height=0, root_hash=""`, and committed height only
> reappeared **+1.0 s later**. So:
> - gate on a non-empty `public_key` — strictly better than a bare 200, and it
>   is what `service run`/`service enable` themselves need (T1-2, T1-3);
> - **never** read `height` or `root_hash` from the first published status. Any
>   step that needs committed state polls for the transition to a non-zero
>   height, and T1-1 records both timings.

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
| **#841** | **a stopped daemon takes its sandbox with it.** One `services::serve_until_stopped` arms SIGTERM/SIGINT for every daemon; teardown removes this instance's containers then stops the podman child. `PodmanService::claim` is a `flock` on `owner.lock`, not an exe-path compare. `/v1/services` carries the node's own build stamp. **Three of §0.5's five known-open bugs are fixed and C-3's three log strings are dead** — §0.5, P-3b, C-3, T1-4, R-3. |
| **#844** | the lent claude TUI no longer lands on the login screen — `prepare_config_home` seeds `hasCompletedOnboarding`, on **every** platform. **T2-6's "Why not the macmini" box cited two env vars and a commit that are not in this tree.** Adds a per-request broker `debug` line (`ducktape::broker`, `brokered request`) — **T2-6, §10.** |
| **#845** | a run's config home is 12 random bytes per run and is removed on drop. No step reads that path; adds one `reason="config_home_not_removed"` WARN — §10. |
| **#846** | **CLI ergonomics, and it defuses T2-5's flag trap.** `--node <not-a-url>` is refused where it was typed instead of dying in reqwest; `-n/--network` is optional on a single-workspace box for `user`/`cred`/`agent`/`fs`/`service`; `user account-init` MINTS `user.key` (and prints a 24-word mnemonic) instead of failing on its absence; `service status <KIND>` parses. — **T2-5, T2-4, P-3b.** |
| **#843** | **DELEGATION SHIPPED.** `SessionRequest` gained a required tagged `work: WorkRef`; a run submitted by A and executed on B draws on A's grant, with B sending only a saga POINTER. **It inverts what §0.4a used to instruct** — and with it T2-4's negative half and §12's honest-list bullet. Six new lender-side refusal tokens (§10). |
| **#847** | the lender **records the draw**: `admit()` logs `airlock session opened credential=… caller=… work=direct\|delegated(…)` at **INFO**, target `ducktape::gateway`. Before it, an admitted session logged **nothing** — T1-7's `grep -c 'credential='` could not pass. `service run <kind>` now also tees `<workspace>/<kind>.log`. — **T1-7, T2-5, T2-6, §10**. |

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
pasta   ~/.local/opt/podman-debian13/root/usr/bin/pasta   (off PATH)
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
- **Delegation SHIPS, and the refusals that remain are narrower than they were.**
  See §0.4a. It is still the section most likely to produce a misfiled report —
  the direction has simply inverted: the risk is now recording a working
  cross-box draw as broken because a step told you to expect a refusal.
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

### 0.4a Delegation: A-submits, B-executes, drawing on A's grant — this WORKS

**[#843, #847] Read this before T2-4, T2-5 or T2-6, or a working feature gets
filed as a defect.** An earlier revision of this section said delegation "does
not exist here" and told the tester to record the cross-box draw's failure as
`EXPECTED-REFUSAL`. That instruction inverted after PR #843 merged
(`c99a9a9c5` + `fad75d469`, in `dev` at `417cbae5c`) and it has been deleted.
Delegation was subsequently **live-verified cross-box**: the dev box submitted
with `--host-node <macmini>`, the macmini executed inside a Tart macOS guest
whose own grant list was empty, and the run returned a real `PONG` drawing on
the SUBMITTER's credential.

**What the executor sends is a POINTER, and only a pointer.** `SessionRequest`
still has no account field — that field was the credential-theft defect #833
deleted, and nothing put it back. It gained one required, tagged member:
`work: WorkRef` (`crates/airlock/src/wire.rs`), with arms `Direct` and
`Saga { saga_id }`. `deny_unknown_fields`, no `serde(default)`, so every
producer states which arm it means. The compute daemon's resolver
(`NodeCredentialResolver::resolve`, `bin/node/src/compute/cred.rs`) passes the
run's `saga_id` through verbatim and reads nothing out of it; the interactive
lane (`airlock_config`, `crates/services/agent/src/lib.rs`) sends
`WorkRef::Direct` because a pty has no committed record of who asked for it.

**The LENDER resolves every fact from its OWN committed state.** `grant_answer`
(`bin/node/src/airlock.rs`) checks the vouched-for caller's own grant first —
unchanged since #833 — and only then, for a `Saga` pointer, enters
`delegated_answer`, which asks its node over `/v1/query` for
`SagaQuery::Get { saga_id }` → `SagaView`. The submitter's authority is proven
by the lender's own consensus view, never asserted by the caller: `/v1/submit`
discards a caller's claimed submitter id and re-signs with the node's own key,
so the committed `SagaOrigin::External(..)` is a signature-proven node key.

**Four conditions, all required, in this order** — the first two are free
(decided on bytes already read) and run before either identity read:

| # | condition | refusal token if it fails |
|---|---|---|
| 1 | the work is still LIVE — `SagaStatus::Pending` | `delegated_work_finished` |
| 2 | the committed spec NAMES THIS CREDENTIAL (`credential_the_work_names`, read through `compute_service::envelope::prepare` — the same producer the CLI composes with) | `delegated_work_names_another_credential` |
| 3 | the vouched caller is the saga's `pinned_assignee` (`caller_is_the_pinned_executor` — the PIN, which is immutable, never the lease, which rotates) | `delegated_caller_not_the_executor` |
| 4 | the saga's `origin` maps to an account the credential admits (`submitter_is_granted`) | `delegated_submitter_not_granted` |

Two more arms on the same path: a pointer over `MAX_WORK_POINTER_BYTES` (512) is
`work_pointer_oversized`, and a `SagaOrigin::Module(_)`/`System` saga names no
account to draw as, so it takes condition 4's refusal. **A saga this lender has
not committed yet is `delegated_work_unseen` → `Undetermined` → 503, which is
NOT a refusal** — see the taxonomy note below.

Conditions 1 and 2 are what `fad75d469` added on top of `c99a9a9c5`, and they
are the difference between lending for a run and lending forever: no terminal
path clears a saga's assignee, so without condition 1 one `Done` run was a
permanent, unmetered draw the owner could not revoke (the executor holds no
grant, so `user cred revoke` has no subject); without condition 2 one lease on
A's saga opened a session for any credential any lender serves that A is granted
on, including a third party's.

#### the three directions, and which verdict changed

| direction | setup | verdict | why |
|---|---|---|---|
| **0** | executor has not admitted the submitter's account | **UNCHANGED — refused** by **work admission**, before the lender is dialled at all. No container, no gateway hop, no session. Token `work_not_admitted`. | Two consents in opposite directions. The executor deciding whose work it runs is a different question from the lender deciding whose account may draw, and #843 touched only the second. |
| **1** | admitted; the credential **OWNER** submits; executor ungranted | **CHANGED — now SUCCEEDS.** `Done`, with `PONG` in the saga result, and the executor still on no grant list. | This is exactly the delegated shape. The owner is always `credential_use_allowed`, the run is `Pending`, its spec names that credential, and the executor holds the pin — all four conditions hold. |
| **2** | grant the **EXECUTOR's** account | **UNCHANGED — succeeds**, and it is now the non-regression half: an executor granted in its own right still draws in its own right, through `Draw::Direct`, with no pointer doing any work. | The caller's own grant is checked first and delegation is purely additive. |

**Which grant a human should actually issue.** For T2-5's topology — the dev box
owns the credential AND submits, the borrower executes — **no `user cred grant`
is required at all.** `credential_use_allowed` (`crates/modules/system/gateway/src/interface.rs`)
is `owner || grantee`, and the submitter here IS the owner. The grant to the
**borrower's** account is what T2-6 needs: an interactive pty sends
`WorkRef::Direct`, has no pointer, and is authorized on the executing node's own
standing or not at all. T2-4 and T2-4b say which is which at the step.

#### what still refuses, and must be recorded `EXPECTED-REFUSAL`

Each of these is a real gate, and a pass that never sees one has not established
that the positives can fail:

- **direction 0** — the executor's `work admit` policy (T2-5's and T2-6's
  deliberate-failure halves). Unchanged by #843.
- **an ungranted, undelegated executor** — the borrower submits a run pinned to
  ITSELF, naming the lender's credential, with no grant: origin, pin and vouched
  caller are all the borrower, so there is no submitter to delegate from.
  `credential_not_granted`. This is the cheapest CLI-level proof that the
  credential gate is real, and it is T2-4's negative half now.
- **an expired pointer** — replay a pointer whose saga has reached a terminal
  status. `delegated_work_finished`.
- **the single-credential bound** — a pointer presented for a credential the
  committed work does not name. `delegated_work_names_another_credential`.
- **a pointer to somebody else's work** — a caller that is not the saga's
  `pinned_assignee`, or an UNPINNED saga (which delegates to nobody).
  `delegated_caller_not_the_executor`.

**The first fails on the saga lane and never dials the lender at all; the other
four collapse on the wire into one `403 credential_not_granted`.** The
granular token exists only in the LENDER's own log, at `debug`, target
`ducktape::gateway`, message `airlock session refused`, carrying nothing but
`reason` (§10). So a tester reading only the borrower's side cannot tell which
condition failed — start the lender's daemon with
`RUST_LOG=ducktape::gateway=debug` (`RUST_LOG` **adds to** the default filter,
it does not replace it: `bin/noded/src/log.rs`, `boot_filter`) before running any
negative half here.

#### `Undetermined` is not a refusal, and the borrower now retries it

The three-state `GrantAnswer` taxonomy is load-bearing and #843 leaned on it
harder, so do not let a report collapse it:

- `Granted` → the session opens.
- `Refused` → **403 `credential_not_granted`**. Settled. Re-asking is a slower 403.
- `Undetermined` → **503 `grant_authority_unavailable`**, which the borrower's
  broker names `airlock_grant_authority_unavailable`. It means *"the lender could
  not ASK"* — a `/v1/query` timeout, a restarting node, a resident not yet
  serving, or a lender a block behind that has not committed this saga yet. It
  **never** means denied, and reporting it as denied sends the operator to add a
  grant they may already hold.

`open_session_retrying` (`crates/services/broker/src/lib.rs`) re-asks **that one
arm and no other**, `SESSION_RETRY_ATTEMPTS = 6` × `SESSION_RETRY_DELAY = 700 ms`
— roughly two block times, sized for the lender-one-block-behind case. It logs
attempt 1 at `debug` and the last at `warn` with an `attempts` field. So on the
delegated lane a single 503 is *expected weather*, not a finding; six of them in
a row is.

#### the merged test that asserts all of this

`a_delegated_run_draws_on_the_submitters_grant` (`bin/node/tests/sched_pinned_run.rs`).
**The name this section used to cite —
`a_delegated_run_draws_as_the_executing_node_not_the_submitter` — no longer
exists anywhere in the tree**; #843 renamed it along with the behaviour, and any
report or doc still naming it is reading a pre-#843 world.

It runs FOUR directions plus a replay on ONE two-node cluster with a real
`service run airlock` lender, a real compute daemon, real containers and a mock
upstream, so exactly one thing differs between any two of them — and its
numbering is its own, not this section's:

| test direction | shape | outcome |
|---|---|---|
| 0 | 0 submits, pinned to 1; 1 admits nobody | `work_not_admitted` |
| 1 | 1 submits, pinned to itself; nobody granted | `credential_not_granted` |
| 2 | 0 submits, pinned to 1; 1 admitted, **still ungranted** | `Done` + `PONG` |
| 2b | direction 2's pointer replayed once its saga is terminal | refused |
| 3 | 1 submits, pinned to itself; 1 granted | `Done` + `PONG` |

The negative constructs are
`cred_lending::granted_credential_resolves_and_round_trips_across_nodes`
(`bin/node/tests/cred_lending.rs`), which drives one positive delegated open and
then four more against the same real lender daemon over the same real overlay:
a pointer to work the caller is not pinned to, a pointer whose committed work
names a different credential, and the same pointer replayed after its saga was
cancelled — all three **403** — plus a pointer naming a saga the lender never
committed, asserted to be **503 `grant_authority_unavailable`** and explicitly
*not* a 403.

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
  (That doc drift is **fixed** in the same PR as this revision:
  `2026-07-26-work-admission.md` §7.1 described the fix as a `MembersOnly` post
  policy with a participants roster. #835 explicitly rejected that route —
  chat's `SetMembership` drops its `author`, so any member can add themselves to
  any roster and post through a members-only policy — and gated at
  `project_message` on `Channel.owner` instead, in
  `bin/noded/src/term_consensus.rs` and nowhere else.)
- **The three `#[ignore]`d provider tests still skip silently**
  (`crates/services/provider/src/lib.rs`:
  `podman_socket_interactive_session_drives_a_tty`,
  `podman_socket_echo_round_trips_through_invoke`, `macos_tart_hardware_smoke`).
  A default run reports them `ignored`, which is honest. They only lie under
  `cargo test -- --ignored` on a host without `DUCKTAPE_PODMAN_SOCKET` or off
  Apple Silicon. **Do not run this pass with `--ignored`**, and if you do, read
  their stdout.

### 0.5 Known-open product bugs — record, do not re-file

The first execution of the credential-free sections found these. **PR #841
(`a6436c486`) has since fixed three of the five and narrowed a fourth**; the
table below is re-verified against `dev` @ `02ad78b5e` and now carries both
verdicts, because a report comparing against an older run needs to know which
line moved.

| bug | status at `02ad78b5e` | what a step sees | steps |
|---|---|---|---|
| **the daemon orphans its podman service on every stop path** | **NARROWED to SIGKILL only [#841]** | SIGTERM/SIGINT are handled now: both daemon bodies enter through `services::serve_until_stopped` (`bin/node/src/services.rs`), which arms the handlers inside `block_on` before the body exists, and both call `stop_sandbox` on the way out — containers first (`sweep_own_containers`), then `PodmanService::shutdown`. Compute additionally races `await_node` against the stop, so a daemon waiting on a down node is SIGTERM-able. **SIGKILL is explicitly not covered and is not papered over.** So rule 4's socket pile-up is now a property of the SIGKILL steps, not of every stop. | C-3, R-4, I-4 |
| **containers are killed rather than re-adopted across restart** | **still open as behaviour; ALL THREE of C-3's strings are DEAD [#841]** | there is no attach path in the tree; the boot sweep destroys what a crash left. But `reaped orphaned sandbox containers` / `reason="own_orphans"` / `reason="reap_failed"` no longer exist anywhere in the tree — see C-3 for the replacements. | C-3, R-4 |
| **the singleton guard is keyed on exe path** | **FIXED [#841]** | `runs_exe` is gone from the tree. `PodmanService::claim` (`crates/services/sandbox/src/podman_api.rs`) now takes an exclusive `libc::flock(LOCK_EX\|LOCK_NB)` on `<root>/owner.lock`, which the kernel releases on death — so a second daemon from a *different* binary path IS refused. Refusal text unchanged (`another service daemon (pid N) already owns …`, `pid` = `unknown` if `owner.pid` is unreadable), plus a new ERROR `reason="sandbox_root_owned_by_another_daemon"` on `ducktape::sandbox`. | T1-4, R-3, P-3b |
| **`service status` prints the CLI's own build stamp** | **FIXED for the human view [#841]** | `/v1/services` carries the node's own stamp; the CLI reads it into `Catalog.node_build` and renders through `Skew::between` rather than its own `build_identity_or_unknown()`. **`--json` does not carry it**: `status --json` prints a bare array of `ServiceRow` and `list` discards the node build outright — so a `--json` "builds agree" read is still not evidence about the NODE. | P-3, R-3 |
| **the fixed 5 s podman-service budget** | **still open, byte-identical** | §0.1 rule 5. `PodmanService::await_socket` is still `for _ in 0..100` × 50 ms then FATAL, and there is no env knob. | T1-4, K-1 |

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
cd <repo>
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

# V-2's inventory, from the same tree that built the binary. The BASELINE is
# the checked-in list in V-2, not a second read of this tree — see V-2.
LOGS="$HOME/wave2qa-logs"; mkdir -p "$LOGS"
v1_routes() {   # multi-line aware, and it reads BOTH router files
  cat bin/noded/src/lib.rs bin/noded/src/admin.rs \
    | tr '\n' ' ' | grep -oE '\.route\( *"/v1[^"]*"' \
    | sed 's/.*"\(.*\)"/\1/' | sort -u
}
v1_routes > "$LOGS/routes.now"
wc -l < "$LOGS/routes.now"        # 44 on this tree — record it
```
**Fail:** a non-empty `git status` (a dirty tree makes `DUCKTAPE_BUILD` a
working-tree digest, which will read as skew in R-3 for the wrong reason).
**Fail:** a route count other than 44 — go straight to V-2 and diff.

**macmini:** same commit, native ARM build. It has no `cargo` by default —
rustup was installed there previously. Binary at
`~/dev/ducktape/target/release/ducktape`.

### P-3 — confirm both boxes are on the same build
**Run on:** both. **Its observable was unreachable and has been replaced.**

`DUCKTAPE_BUILD` is stamped at compile time by `bin/noded/build.rs` from the
commit plus a working-tree digest when dirty. It is `option_env!`, so **setting
it at runtime does nothing**.

> **What this step used to say, and why it could not be done.** It asked for the
> stamp at precondition time. **There is no precondition-time reader for it.**
> `ducktape --version` prints `ducktape 0.1.0` — the crate version, never the
> build stamp — and `DUCKTAPE_BUILD` is exposed nowhere else on the CLI.
> `build_identity()` (`bin/noded/src/services.rs`) is only ever read into a
> `Hello`, the `/v1/services/hello` 200 body, and `service status`'s `build`
> row, all of which need a **live node** and, for `service status`, a
> **signaling daemon** as well — none of which exist yet at P-3.

**So this step is now two halves, and the second one is deferred by design.**

**a. at P-3 — the SHA and the boot stamp.** The commit is the thing both boxes
must agree on; the build stamp is derived from it.
```bash
git -C . rev-parse HEAD          # both boxes: the SAME sha
git status --porcelain | head    # both boxes: EMPTY (a dirty tree ≠ that sha)
./target/release/ducktape --version   # `ducktape 0.1.0` — record it, it is NOT the stamp
```
**Fail:** different SHAs, or either tree dirty. Fix before proceeding; every
later skew assertion becomes meaningless.

**b. deferred to T1-4 — the stamp itself, off a live node.** The first moment
the stamp is readable is when a daemon is signaling:
```bash
"$D" service status compute -n "$CHAIN" --json | python3 -c 'import sys,json;print(json.load(sys.stdin)["build"])'
```
plus the node's own boot line, which carries the *derivation* and not the stamp:
`INFO ducktape::node: … version=… profile=release binary=<path> built_unix=<epoch>`
(`bin/node/src/boot/env.rs`). **Record `built_unix` and `binary` from both
boxes** — with an identical SHA and a clean tree those are the honest
cross-box evidence available before T1-4.

> **[§0.5 known-open] `service status` can render the CLI's OWN stamp.** The
> `build` row is meant to come from the catalog's hello (`live.build`,
> `bin/node/src/services.rs`), but the CLI also stamps
> `build_identity_or_unknown()` into its own hello on the same path. Until that
> is fixed, **"builds agree" in `service status` is not evidence** — read R-3's
> log line instead, which is a round trip through the node's 200 body.

### P-4 — reap the box's leaked state: processes, sockets, **and storage roots**
**Run on:** dev box.

**This step used to lie.** It reaped processes and sockets and called the box
clean, while leaving every test's **storage root** on the tmpfs. Both halves
matter, and the second one is the half that fills `/tmp` (§0.3).

**a. processes. The pass criterion this step used to carry was itself a §0.1
rule-1 violation.** `pgrep -u "$USER" -f -c 'podman.*system service'` returned
**2** on a box with **zero** real podman processes: both hits were the shell
that ran the command. A count that can never reach 0 is not a pass criterion —
it is a step that always fails, which is the same defect as one that always
passes. **Count and assert with `pgrep -x podman`**, and read each hit's
identity out of `/proc`.

The PR test suites launch `podman system service --time=0`, which never
idle-timeouts; 102 orphans parented to `init` were counted on this box in one
day. **[§0.5 known-open] the daemon also orphans its own podman service on
every stop path**, so this dir grows during the pass, not only before it.

Reap by verified identity, never by pattern:

```bash
ls /run/user/1000/ducktape/ | wc -l
pgrep -u "$USER" -x podman | wc -l          # the REAL count. -x, never -f.

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

**b. storage roots — plain `rm -rf` does not finish the job.**
**`podman unshare` is still mandatory. The reasoning and the numbers this step
used to give were both wrong, in a way that mattered.**

Rootless podman chowns its overlay graph root into a **user namespace** (the
subuid range), and the unmapped user cannot traverse or unlink some of those
directories. What that actually does — measured twice, independently:

| the old claim | what `rm -rf` really does |
|---|---|
| "exits 0, reporting success" | **exits 1**, with ~40 `Permission denied` lines. It is **loud**, not silent. |
| "~830 MB of an 845 MB store survived" | it removes ~**98%** of the bytes: 253 M/8236 files → **5.2 M/60 files**; 237 M → **5.3 M** on a second box. A small residue survives and the directory is left standing. |
| "`du` is blind to it" | **`du` is not blind.** Inside and outside the userns report the **same** size, so a `du` post-check is sound — and it is the post-state this step asserts on. |

Why it still matters, and why the fix is unchanged: a 5 MB residue with the
directory still standing is **not** an empty image store. K-1 restarts a daemon
onto that root and reads its image list; the residue is exactly what makes a
"cold" start ambiguous. So:

```bash
podman unshare rm -rf <path>
```

`podman unshare` re-enters the same user namespace the storage was created in,
where those directories are owned by root and removable. **The exit code is not
the evidence either way — `du` is.** Sweep the leftovers:

```bash
# the graph roots the suites leave behind: <data>/podman/{storage,run,hooks}
for d in /tmp/.tmp*/ /tmp/dt-svc-check/; do
  [ -d "$d" ] || continue
  podman unshare rm -rf "$d"
done
```

**Observable / pass — a post-state, looked at, not an exit code:**
```bash
pgrep -u "$USER" -x podman | wc -l                # -> 0   (-x; `-f` matches this shell)
ls /run/user/1000/ducktape/ | wc -l               # -> 0
ls -d /tmp/.tmp*/ 2>/dev/null | wc -l             # -> 0
du -csh /tmp/.tmp* /tmp/dt-svc-check 2>/dev/null | tail -1   # -> nothing, or 0
df -h /tmp | tail -1                              # record the free figure
```
**Neither `rm -rf`'s exit code nor `podman unshare`'s is the pass** — a bare
`rm -rf` exits **1** having removed 98%, and that is a pass-shaped failure in
both directions. Re-run `du` and read the number.
**Fail:** a survivor whose cmdline points at a **workspace** (not `/tmp`) — that
is a live node's service; investigate, do not kill.

**c. the same shape, everywhere else in the teardown.** Every step that destroys
state must name what it looks at afterwards. Swept for this revision:

| step | was | now |
|---|---|---|
| **K-1** ("make the image store genuinely empty") | `rm -rf "$WS/…/podman/storage"` — **the same rootless-overlay path**, so the store was never emptied and the cold-start step proved nothing | `podman unshare rm -rf`, then `du -sh` the path and assert the image list is empty |
| **I-3** (airlock SIGTERM) | already asserts the route is **gone** from `gateway-routes.json` and the file **removed** if it was the only one | kept — this one was already a post-state |
| **I-4** (SIGKILL blast radius) | "poll for the state string" | kept, and the container list is re-read on both sockets |
| **V-2** (`/v1` additive only) | a route inventory diffed against a baseline taken **from the same immutable checkout**, so the diff was empty by construction — and the extractor missed 11 of 44 routes | baseline is now the **checked-in list in V-2**, and the extractor is multi-line-aware and reads `admin.rs` too |
| **V-3** (no secrets in logs) | greps expecting empty output, and a pattern blind to both secrets this runbook mints | asserts the log files are **non-empty first**, and matches bare `[0-9a-f]{64}` |
| **V-5** (no daemon holds the node key) | an fd probe that reads **0 for the node itself** | the structural lint test is the criterion; the fd probe is demoted to a note |

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
    agent-workspaces/  term-sessions/  forge-repo/
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
**Run on:** both. **Its edit instruction was self-defeating and is deleted.**

> **DO NOT APPEND ANYTHING. `node init` already wrote this table.** Confirmed
> independently by two agents on the first pass: `cli.rs`'s
> `detect_platform_sandbox()` runs on `node init` **and** `node join`, probes the
> platform adapter, and — when it resolves — writes a **byte-identical** table:
> `runtime = "podman"`, `image = DEFAULT_PODMAN_IMAGE`
> (`docker.io/library/node:22-slim`), `cores = 0`, `mem_gb = 0`
> (`bin/node/src/config/resolve.rs`). It says so on stderr at init time:
> `compute plane: podman found at <path> — writing a live [sandbox] table`.
>
> **Appending the old block verbatim produced a third state the step did not
> describe:**
> ```
> FATAL: "<ws>/node.toml": … TOML parse error … duplicate key `sandbox`
> ```
> — which contains **neither** the step's fail assertion (`SandboxToml`) **nor**
> its pass assertion (`compute_not_granted`). A literal follower lands somewhere
> the runbook has no verdict for, and the most likely reading is "the node is
> broken".
>
> **P-1 is what makes this work.** `detect_platform_sandbox` writes the table
> only if `SandboxBackend::probe()` succeeds, and probe needs **podman + pasta +
> nft + nsenter**. A `node init` run in a shell that skipped P-1's `PATH` export
> silently writes **no** table — which is the one case where you do add one.

**So this step is now a check, not an edit:**

```bash
grep -n -A5 '^\[sandbox\]' "$WS/node.toml"
grep -c '^\[sandbox\]' "$WS/node.toml"      # MUST be exactly 1
```

| what you see | what to do |
|---|---|
| exactly one `[sandbox]` with the four keys above | **nothing.** This is the pass. |
| **no** `[sandbox]` at all | P-1 was not in the shell that ran `node init`. **Re-run `node init` with the P-1 exports** — do not hand-add the table, because a hand-added one hides the fact that probe failed, and the daemon will then die at `sandbox: <detail>` in T1-2 with no explanation. |
| two `[sandbox]` headers | someone appended. Delete the appended one. |

macOS (Tart) writes the platform's own values, also at `init`:
`runtime = "tart"`, `image = "ghcr.io/cirruslabs/macos-sonoma-base:latest"`.
`cores`/`mem_gb` are `0` there too (`0` = probe the host); the old runbook's
`cores = 2 / mem_gb = 4` was a hand-edit, not what init writes.

**The deliberate-failure half — this is where the flat spelling belongs.**
`NodeToml.sandbox` is `Option<SandboxToml>` with `deny_unknown_fields`, so the
**retired flat spelling** is a serde *type* error, not a bespoke message. On a
**copy** of the workspace, replace the whole table with `sandbox = "podman"` (do
not append — that is the duplicate-key state above) and boot:
```
FATAL: "<path>/node.toml": … invalid type: string "podman", expected struct SandboxToml
```
**Assert on the substring `FATAL:` plus `SandboxToml`**, not on a full sentence —
the span rendering is `toml` 0.8's and is not pinned by any test. If you edit a
table by hand for any reason, its header must be **last** in `node.toml`
(everything after a TOML table header belongs to it).

**Observable / pass:** the node boots and, with no compute grant yet, logs
exactly:
```
WARN ducktape::service: sandbox configured but the compute service is not enabled; this node will run no provider work and announce no capabilities — enable it with `ducktape service run compute` … reason="compute_not_granted"
```
That warn is the proof the table parsed.
**Fail:** `FATAL: … SandboxToml` on the unmodified workspace (the flat spelling
leaked in), or `duplicate key` (someone appended), or **no warn at all** (no
table — the middle row of the table above).

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

cargo test -p node-bin --test dispatch_e2e 2>&1 | tee "$LOGS/dispatch.out"
grep -c '^SKIP ' "$LOGS/dispatch.out"                # MUST be 0
grep -c 'test result: ok' "$LOGS/dispatch.out"       # MUST be >= 1
grep -cE '^test .* \.\.\. ok$' "$LOGS/dispatch.out"  # MUST be >= 1 — a binary with
                                                     # ZERO tests also prints "ok"
```

> **A third check used to sit here and was unsatisfiable by construction:**
> `grep -c 'compute daemon serving' "$LOGS"/*.log` "MUST be >= 1". At
> precondition time **no daemon has started in `$LOGS` at all** — the first one
> is T1-4 — and `dispatch_e2e` spawns its own daemons into the **test's** temp
> root, never `$LOGS`. It could only ever have been 0. It is deleted; the
> `test result: ok` + per-test-`ok` pair above is what actually distinguishes
> "ran something" from "built a binary with zero tests", which is the vacuum
> #831 documented.

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

## 2. Tier 1 — single-box credential plane (T1) — 8 steps

**This section used to be titled "no airlock at runtime", and that claim is
false on this tree.** Every `agent sched --cred` run — including one drawing on
the operator's **own** credential, on the **same** box — resolves through
`AirlockConfig::self_host` and dials `airlock.<own-handle>.duck` over its own
gateway proxy. There is no local branch (T1-7's head note traces the path).

**The claim actually under test, restated:** *broker-host is always in the path
(per-run loopback + opaque bearer), and a single box can lend to itself* — the
same topology T2 runs across two boxes, collapsed to one. `service run airlock`
is a **precondition** of T1-7, not a Tier 2 escalation.

Topology for this tier: **dev box alone.** One node; compute, agent **and
airlock** daemons.
**Step order changed:** T1-1 → T1-2 → T1-3 → T1-4 → T1-5 → T1-6 → **T2-2** →
T1-7 → T1-8.

### T1-1 — found the network, node up

```bash
export DUCKTAPE_HOME="$HOME/.ducktape-wave2qa"
export PATH="$HOME/.local/opt/podman-debian13/root/usr/bin:$PATH"
export TMPDIR="$HOME/wave2qa-tmp"
D=<repo>/.worktree/wave2-qa/target/release/ducktape

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

**Record, do not fail on — and it is NOT the pre-publish window any more.** That
window is closed (§0.1's correction): two boots, ~750 polls, zero 200s with an
empty `public_key`. **The timing worth capturing here is the one that is still
open** — the gap between the first published status and the first status
carrying a **committed height**. Measured on a warm restart: the first 200
carried the key with `height=0, root_hash=""`, and committed height reappeared
**+1.0 s later**. On a genesis boot: 755 connection-refused polls, then the
first 200 already had the key.

```bash
# poll fast, from the moment the process starts, and print the two transitions.
python3 - <<'PY'
import json,time,urllib.request
t0=time.time(); pub=None
while time.time()-t0 < 120:
    try: s=json.load(urllib.request.urlopen("http://127.0.0.1:9971/v1/status",timeout=1))
    except Exception: time.sleep(0.005); continue
    if pub is None and s.get("public_key"): pub=time.time()-t0; print(f"published  +{pub:.3f}s")
    if pub is not None and s.get("height",0) > 0:
        print(f"committed  +{time.time()-t0:.3f}s  (gap {time.time()-t0-pub:.3f}s)"); break
    time.sleep(0.005)
PY
```
Report both numbers. **A non-zero gap is expected and is exactly why no step may
read `height`/`root_hash` off the first published status** (§0.1).

### T1-2 — compute signals, and is refused nothing

```bash
RUST_LOG=info,ducktape::service=debug,ducktape::saga=debug \
  "$D" service run compute -n "$CHAIN" --no-enable \
  > "$LOGS/compute.out" 2> "$LOGS/compute.log" &
```
`--no-enable` is deliberate: it proves the non-TTY path emits **one line** and
keeps serving, which is what a systemd unit does.

**Observable / pass:**
- `compute.log` (stderr) carries the banner:
  `● compute · signaling to <CHAIN> · offering <tags>`
- then **two** hint lines, in this order — `--no-enable` is `EnableOffer::Never`,
  which writes the first from `offer_enable` and the second from `serve_kind`
  (`bin/node/src/services.rs`):
  1. `not enabled — enable it with: ducktape service enable compute`
  2. `not enabled — nothing will execute until it is enabled`
- **no prompt, no spinner, no re-ask** — grep the file for `[Y/n]`: must be 0 hits.

> **CORRECTION: "no ANSI escapes because stderr is a file" is false.** Only the
> two `write_err` hint lines and the `●` banner are plain — and even those carry
> SGR around the yellow `not enabled`. The *tracing* lines in the same file
> **do** carry escapes: `log::init` applies `.with_ansi(false)` to the ring layer
> and the daemon.log file layer but **not** to `stderr_layer`
> (`bin/noded/src/log.rs`), and a `service run` daemon has only that layer
> (P-7). So:
> ```bash
> grep -c $'\033\[' "$LOGS/compute.log"     # NON-ZERO is expected, not a finding
> sed 's/\x1b\[[0-9;]*m//g' "$LOGS/compute.log" > "$LOGS/compute.plain.log"
> ```
> **Strip escapes before every grep in this runbook that matches a coloured
> token**, or a `grep -c 'reason="build_skew"'` can miss a line that is there.
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
**Record that `instance=` value — it is R-1's observable.**
Its own podman service is up and answers:
```bash
# [#841] owner.lock is new — it is the flock the singleton guard now holds
ls "$WS/storage/services/compute/podman/"        # storage run hooks owner.lock owner.pid podman.pid
CSOCK=$(own_sock compute) || exit 1              # §0.1 rule 4 — NEVER the glob
curl -s --unix-socket "$CSOCK" http://d/_ping    # -> OK
```
**Also do P-3b here** — this is the first moment the build stamp is readable:
```bash
# [#846] `service status <KIND>` parses now (it used to answer
# `error: unexpected argument 'compute' found`), and -n is optional on a
# single-workspace box. [#841] --json is an ARRAY of rows, so index it.
"$D" service status compute -n "$CHAIN" --json \
  | python3 -c 'import sys,json;print(json.load(sys.stdin)[0]["build"])'
```
> **[#841] that `--json` value is the DAEMON's stamp, not the node's.** `--json`
> prints a bare array of `ServiceRow` and carries no node-build key at all;
> `list` discards it outright. The node's own stamp reaches only the **human**
> `service status` render, through `Skew::between`. So the "builds agree"
> reading has to come from the human view — a `--json` read cannot make it.

**Fail:** `another service daemon (pid <N>) already owns <socket> — stop it
before starting this one` → a previous daemon survived; use `stop_by_config`,
never a pattern kill. **[#841] that guard is a `flock` now, not an exe-path
comparison**, so it DOES fire between two daemons started from different binary
paths (R-3's second binary included) and the kernel releases it on death. A
second structured line names it: ERROR, target `ducktape::sandbox`,
`reason="sandbox_root_owned_by_another_daemon"`, `holder=<pid|unknown>`.
**Fail:** `podman service did not answer on <path> within 5s` — **but read
§0.1 rule 5 before recording it.** Re-run the same start alone and report
`FLAKE(load=<avg>, alone=<secs>)` vs a real finding.

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
- two podman services now, two graph roots — resolved by identity, **not** by
  globbing the shared runtime dir (§0.1 rule 4):
  ```bash
  ls "$WS/storage/services/"                     # agent  compute
  ASOCK=$(own_sock agent) && CSOCK=$(own_sock compute) && echo "$ASOCK" "$CSOCK"
  test "$ASOCK" != "$CSOCK"                      # two DISTINCT paths
  ```

> **[#829] the auto-enable path does not die on a failed announce.** `service run
> --enable` downgrades a failed commit to a warning with
> `reason="enable_not_announced"` and keeps signaling. So a green
> `agent daemon serving` does **not** by itself prove the announce landed —
> grep for that reason token before trusting T2-3.

**Fail:** `reason="link_refused"` with
`refused: present this node's service-link token, and only one agent service may
attach` → the token was stale (the node restarted) or a second agent is running.

### T1-6 — an operator-owned credential, and the airlock route it publishes
**Retitled. "No airlock in the path" was the tier's premise and it is wrong —
see the correction at the head of T1-7 before running either step.**

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
"$D" user account-init --name alice -n "$CHAIN"     # password on stdin
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

**And it published the airlock route.** `cred add` writes the on-chain `airlock`
RouteStatement whether or not an airlock daemon ever runs. That route is the
thing T1-7's runs will dial — read T1-7's head note now, not after a red run.

### T1-7 — headless run, end to end

> ## The tier's headline claim does not survive the code, and this step is
> ## where it fails. Read this before running it.
>
> Tier 1 was written on the claim *"airlock is a credential SOURCE, not a
> dependency — the operator's own credential resolves locally and never touches
> an airlock."* **On this tree there is no local branch.** `agent sched`
> **hard-requires** `--cred <NAME>` (`SchedArgs.cred: String`,
> `bin/node/src/agent_cli.rs`), and every named credential goes through one
> path with no alternative:
>
> `NodeCredentialResolver::resolve` (`bin/node/src/compute/cred.rs`) →
> `build_airlock` → `AirlockConfig::self_host(&ResolvedCredential { authority:
> "airlock.<owner-handle>.duck", via: <this node's browser gateway>, … })` →
> `AirlockGateway::Remote`. There is **no arm** that returns a non-airlock
> config. The credential's own owner, on the same box, still dials
> `airlock.<own-handle>.duck` through its own gateway proxy.
>
> **Observed on the first pass:** with no airlock daemon running, **every**
> `agent sched --cred` run failed `airlock_route_or_credential_absent` — the
> gateway resolved the route T1-6 published, found no local loopback upstream in
> `gateway-routes.json`, and returned 404 (`proxy_loopback`,
> `bin/node/src/gateway_plane.rs`).
>
> **So T1-7's old pass criterion — "that grep must be 0" — asserted the opposite
> of what the topology does.** It is inverted below. The genuinely airlock-free
> path is `agent pty` with **no** `--cred`, which falls through to
> `AnthropicAuth::from_host()` and reads the operator's live
> `~/.claude/.credentials.json` — exactly what T1-6's HOLD forbids. That is the
> whole reason the claim looked true and never was.
>
> **CAVEAT — re-verify when the real credential lands.** The observation above
> was made with a **fabricated** credential imported through
> `DUCKTAPE_CRED_REUSE_ARTIFACT`, which validates nothing (§0.1 rule 6). The
> *refusal token* is a routing fact and does not depend on the credential being
> real — but **re-run this step against the user-supplied throwaway and confirm
> the token before writing it up as settled.** Record it as
> `FINDING(needs re-verification)` until then.
>
> **This does not sink Tier 1.** It relocates it: T1-6 → T1-7 is exactly the
> topology T2 builds, one box instead of two. Run T2-2 (`service run airlock`)
> **before** T1-7 so the route it published has a listener, and T1-7 becomes
> "the operator lends to itself, over its own gateway" — which is a real and
> testable claim. What it is **not** is "no airlock at runtime".

**Ordering, changed:** run **T2-2 (airlock daemon on this box)** before this
step. Then:

```bash
RUN=$("$D" agent sched claude --cred alice-claude-1 -n "$CHAIN" --cpu 1 --mem 2 -- "reply with exactly: PONG")
echo "$RUN"        # -> sched<0x1F><32 hex>   NOTE: literal ASCII unit separator
DISPATCH=${RUN#*$'\x1f'}
SAGA="$RUN"        # the saga id IS the whole string, separator included
```

**Wait on — the events, in order:**
1. `await_line "$LOGS/compute.log" 'compute daemon serving'` (already true)
2. **the saga transitions. `pending_runs`/`recent_runs` are the WRONG read —
   see the box below.**
   ```bash
   saga_get() {  # saga_get <saga id, unit separator and all>
     python3 - "$1" <<'PY'
import json,subprocess,sys
q = {"target":"saga","query":{"get":{"saga_id":sys.argv[1]}}}
out = subprocess.run(["curl","-s","http://127.0.0.1:9971/v1/query",
      "-H","content-type: application/json","-d",json.dumps(q)],
      capture_output=True,text=True).stdout
v = json.loads(out).get("saga") or {}
print(v.get("status"), "attempt=", v.get("attempt"), "assignee=", bool(v.get("assignee")))
print("result:", bytes(v.get("result") or []).decode("utf-8","replace")[:400])
print("error:", v.get("error"))
PY
   }
   saga_get "$SAGA"     # poll THIS for the transition Pending -> Done
   ```
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

> ### `pending_runs` and `recent_runs` do not see a `sched` run. Do not use them.
>
> Measured on the first pass: **both returned `[]` for every run placed** —
> including one whose saga had committed `Done` carrying `PONG`. It is not a
> lag and it is not a defect; they read a different lane. `agent sched` submits
> `saga::SagaMsg::Trigger { saga_id: "sched\x1f<dispatch>", reply_to: None, … }`
> (`bin/node/src/agent_cli.rs`). `RunsQuery::PendingRuns` projects the *dispatch
> correlation* table and `RecentRuns` the *delivered-runs ring*
> (`crates/modules/apps/runs/src/module_impl.rs`) — both fed by the delivery
> path a `reply_to: None` trigger never enters.
>
> **The outcome is readable only through `saga` `get`, on the full
> `"sched\x1f" + <dispatch>` string** — the unit separator is part of the id.
> As written, the old step left a tester two choices, and §0.1 rule 3 forbids
> the second: record FAIL on a run that succeeded, or fall back to "no error
> appeared".

**Observable / pass:**
- `saga_get "$SAGA"` reports `status: Done` and its `result` decodes to text
  carrying **`PONG`**. That is the step's primary evidence — nothing else here
  substitutes for it.
- **the same `PONG` crossed the run-output ring** — the second positive half,
  and it is what stops the step passing on a committed-state read alone
- a real container ran: a container labelled `io.ducktape.managed=compute#…`
  appeared on `$(own_sock compute)` during the run (C-1's query)
- `"$LOGS/compute.log"` has **no** `reason="worker_error"`
- **the airlock refusal set is empty *because a listener answered*, not because
  nothing dialled** — the inverted assertion:
  ```bash
  grep -cE 'airlock_gateway_unreachable|airlock_route_or_credential_absent|gateway_seal_pk_mismatch|airlock_caller_account_unverified' \
    "$LOGS"/*.log                       # -> 0
  grep -c 'airlock daemon serving' "$LOGS/airlock.log"        # -> 1  (T2-2 ran first)
  grep -c 'airlock session opened' "$LOGS/airlock.log"        # -> >= 1: the gate ADMITTED
  grep -c 'work=direct' "$LOGS/airlock.log"                   # -> >= 1: on the operator's own standing
  ```
  A zero in the first grep with a **zero** in the third is not a pass — it means
  the run never reached the gateway, and on this topology that cannot happen for
  a `--cred` run.

  > **[#847] this assertion was unsatisfiable until PR #847 and is now the
  > cheapest evidence in the step.** It used to read
  > `grep -c 'credential=' "$LOGS/airlock.log"  # -> >= 1: the gate DECIDED`.
  > Before #847 `grant_answer` returned `GrantAnswer::Granted` with **no
  > `tracing` call at all**, and the refusal writer carries no `credential`
  > field — so on a successful run that grep could only ever return 0, and on a
  > failed one it still would. #847 routes both `Granted` returns through one
  > `admit` writer at `info`. This is the same class of defect as the eight
  > steps the first execution found that could not fail: an assertion whose
  > evidence the product never emitted.

**Deliberate-failure half — the airlock dependency, proved in one stop.** Stop
the airlock daemon (`stop_by_config "service run airlock"`) and submit the same
run again.
**Pass:** it fails, and the borrower-side token is
**`airlock_route_or_credential_absent`** (HTTP 404 — the route is still
published on chain but its local loopback upstream is gone from
`gateway-routes.json`; see I-3 for why this is 404 and not 502). Record
`EXPECTED-REFUSAL(T1-7)`. Restart the airlock daemon before continuing.
**Fail:** the run succeeds — then a non-airlock credential path exists that the
code review above did not find, and **that** is the headline finding.

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

### T1-8 — interactive pty session on the operator's own credential
**Retitled: "airlock-free" was the same wrong premise as T1-7's.**

```bash
"$D" agent pty claude --cred alice-claude-1 -n "$CHAIN" --cpu 1 --mem 2
```

> **`--cred` is now explicit here, and that is the change.** `agent pty`'s
> `--cred` is `Option<String>`, so the old command line was legal — but a pty
> with **no** credential does not take some airlock-free happy path: it reaches
> the broker with `AirlockConfig` absent, falls through
> `resolve_anthropic_upstream(None)` to `AnthropicAuth::from_host()`, and reads
> `ANTHROPIC_API_KEY` / `CLAUDE_CODE_OAUTH_TOKEN` / the operator's **live**
> `~/.claude/.credentials.json` (`crates/services/broker/src/lib.rs`). That is
> the host login **T1-6's HOLD forbids this pass from touching**, and it is
> exactly the path that made "no airlock at runtime" look true.
>
> So: pass `--cred`, and this step goes through the same self-host airlock hop
> T1-7 does — T2-2's daemon must be running. **If you want the credential-less
> variant recorded, record it as `NOT-RUN(would read the operator's live
> login)`.**

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
**Run this BEFORE T1-7, on the dev box.** T1-7's head note explains why: every
`agent sched --cred` run dials `airlock.<owner-handle>.duck` through its own
node's gateway proxy, including one drawing on the operator's own credential on
the same box. Without this daemon there is no listener behind the route T1-6
published, and the run 404s. This step is a Tier 1 precondition that happens to
also be Tier 2's first step.

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
**Two ways in, and this step issues only one of them. Read §0.4a first.**

```bash
# borrower box — [#846] this MINTS user.key when absent; no `user key init` first
"$D" user account-init --name duke -n "$CHAIN"
# dev box (lender)
"$D" user cred grant alice-claude-1 <borrower-account-hex> -n "$CHAIN"
```

> **[#846] `account-init` on a fresh workspace now prints a 24-word mnemonic
> before it submits anything** (`a new user key was minted at <path>` +
> `write these 24 words down — they are the only way to restore it:`). It is a
> secret: it does not go in the report, and it does not go in a shared terminal
> transcript. A new failure mode replaces the old missing-file FATAL — if the
> minted key does not re-open under the password that sealed it, the CLI
> **removes it** and says so rather than leaving an unusable file.

**Since #843 an account is granted TWO ways, and only the first is what this
command issues:**

1. **the caller's own standing** — any workload on a node bound to a granted
   account may draw. This is what `cred grant` writes, and it is checked first
   (`credential_use_allowed`, then `Draw::Direct`). It is what **T2-6's pty
   needs**: an interactive session sends `WorkRef::Direct`, has no committed
   record of who asked for it, and is authorized on the executing node's own
   standing or not at all.
2. **delegation** — a run an account SUBMITS may draw while it executes on the
   node that account pinned it to, for the credential the committed work names,
   until the run reaches a terminal status. Nothing is issued for this; it is
   resolved by the lender from committed state per run.

**So `--host-node` lets that node spend your subscription for that one run**,
which is what `agent sched --cred`'s own help now says. The pin is what scopes
it: the pinned node is the only one that may present the run as its reason for
opening a session.

**T2-5 does not need this grant at all** — the dev box owns the credential and
submits, and `credential_use_allowed` is `owner || grantee`, so the delegated
path admits the borrower on the owner's own standing. Issue it anyway, here, for
T2-6.

**Observable / pass:**
- the `gateway` module's credential record lists the **borrower's** account as
  grantee, and `cred grant` reports `granted at height <N>`
- **Do not assert that any scope is enforced** (§0.4).

**Deliberate-failure half — the cheapest CLI-level proof the credential gate is
real.** Before issuing the grant, on the **borrower**, submit a run pinned to
ITSELF that names the lender's credential:

```bash
# borrower box, BEFORE `user cred grant`
"$D" agent sched claude --cred alice-claude-1 -n "$CHAIN" -- "reply with exactly: PONG"
```

Origin, pin and vouched caller are all the borrower, so there is no submitter to
delegate from and no standing grant either. **Pass:** the saga reaches `Failed`
carrying `credential_not_granted`; record `EXPECTED-REFUSAL` and cite §0.4a.
**Fail:** it succeeds — then the lender is authorizing something neither consent
covers, and that is the headline finding.

> **The old negative half here was the inverse of this, and it was wrong.** It
> said to grant the lender's own account and run T2-5, asserting
> `credential_not_granted` "even though the credential's owner submitted". That
> is precisely the shape #843 made WORK (§0.4a, direction 1). Running it today
> yields `Done` + `PONG`, and a tester following the old text would file the
> feature as the defect.

### T2-4b — admit the submitter's work on the EXECUTING node

**A credential grant and a work admission are two consents in OPPOSITE
directions, and conflating them is the first thing to get wrong.** T2-4 is the
LENDER saying *which account may draw on my credential*. This step is the
EXECUTOR saying *whose work I will run at all*. A cross-node run needs both, on
different boxes — the grant on the dev box, the admission on the borrower.

Since #838 a node runs only its OWNER's work and its own by default, and these
two boxes are two accounts (`alice` on the dev box from T1-6, `duke` on the
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
"$D" agent sched claude --cred alice-claude-1 --host-node duke -n "$CHAIN" -- "reply with exactly: PONG"
```

> **[#832] THE FLAG TRAP — and [#846] it now fails cleanly.** `agent --node`
> used to mean "which peer runs the work". It means **an http base url**, and the
> peer-targeting flag is `--host-node`. `--node duke` still **parses** and still
> **outranks `-n "$CHAIN"`** — `NodeAddr::ladder_rung` is unchanged — so the name
> still never reaches `resolve_host_node`. What changed is the bottom of
> `rung_base`: `Rung::Flag` now goes through `checked_base`
> (`bin/node/src/cli_args.rs`) and is refused **where it was typed**, verbatim:
>
> ```
> FATAL: --node is an http base url, and "duke" is not one (expected http://host:port) — for a network name use: -n/--network "duke"
> ```
>
> `DUCKTAPE_NODE=duke` gives the same sentence naming `DUCKTAPE_NODE`. **The old
> reqwest `builder error` is gone**, so a report still quoting one is from a
> pre-#846 run. `Rung::Context` (a `.duckfs`-recorded url) is deliberately NOT
> checked. The ladder is `--node <http-url>` → `-n/--network <chain-id>` →
> `DUCKTAPE_NODE` → caller context → lone registered workspace, and trailing
> slashes are trimmed on every rung.
>
> **[#846] `-n "$CHAIN"` is now optional on a single-workspace box** — for
> `user`, `user cred`, `agent`, `fs` (via `Rung::LoneWorkspace` /
> `WorkspaceSource::LoneRegistered`) and for `service` (via its own
> `WorkspaceArgs::dir()`). **Keep it in every command below anyway.** With two or
> more registered workspaces all of them refuse with a list of chain ids, and a
> QA box that ran `node init` twice — R-3's second binary, a re-init — is exactly
> that box.

**Observable / pass:**
- the run executes **on the borrower** and returns `PONG`, read the same way
  T1-7 reads it: `saga_get "$SAGA"` on the **submitting** box reports
  `status: Done` with `PONG` in the decoded `result`. **`pending_runs` /
  `recent_runs` will be `[]` here too** — see T1-7's box; they are the wrong
  lane for a `sched` saga on either box.
- the borrower's broker opened a sealed airlock session: **zero** refusal
  tokens in the borrower's logs. A single
  `reason="airlock_grant_authority_unavailable"` with `attempts=1` at `debug` is
  **not** a refusal and not a finding — it is the lender being a block behind,
  and the delegated lane re-asks it (§0.4a). Six of them ending in a `warn` is.
- **[#847] the lender's `airlock.log` carries the DRAW, at `info`:**
  ```
  airlock session opened credential=alice-claude-1 caller=<8 hex> work=delegated("sched\u{1f}<id>")
  ```
  target `ducktape::gateway`. `work=delegated(..)` — **not** `work=direct` — is
  the assertion: it is the one field that separates a caller spending its own
  entitlement from one spending the submitter's, and `direct` here would mean
  T2-4's grant is doing the work and delegation is untested. `caller=` is a
  4-byte account prefix, never the whole account; the credential is named
  because by this line it has been matched against a record consensus committed.
  This line is `info`, so it is visible at the default filter, and #847 also
  makes `service run <kind>` tee `<workspace>/<kind>.log` — so it survives the
  terminal that launched the daemon and is greppable there as well as in
  `$LOGS/airlock.log`.
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
"$D" agent pty claude --cred alice-claude-1 --host-node duke -n "$CHAIN"
```

> **Why not the macmini — and [#844] the reason this box used to give is
> wrong.** It cited `HOST_CREDS_FILE` + `PROVIDER_MANAGED_BY_HOST` (commit
> `b3d95723e`) as the missing plumbing. **Neither name exists anywhere in the
> tree**, and `b3d95723e` is **not an ancestor of `02ad78b5e`** — its only file,
> `crates/modules/system/capability-host/src/lib.rs`, no longer exists either.
> Do not file a bug naming them; the mitigation it points at is not there to
> cite.
>
> **The "TUI asks for a login method" symptom itself is CLOSED, on every
> platform.** #844 found the root cause and it was not the broker: Claude Code
> runs its first-run wizard whenever `<CLAUDE_CONFIG_DIR>/.claude.json` does not
> say onboarding is done, and `prepare_config_home` materializes that config home
> fresh for every run — step two of the wizard is the login prompt, which is why
> only the interactive lane broke while headless `claude -p` worked on the same
> plumbing. `prepare_config_home`
> (`crates/services/provider/src/lib.rs`) now seeds
> `.claude.json` = `{"hasCompletedOnboarding":true}` and `settings.json` =
> `{"skipWebFetchPreflight":true}`, gated only on
> `isolation.broker == Some(BrokerKind::AnthropicMessages)` — **no `cfg(target_os)`
> and no runtime platform branch.** #846 seeds `hasTrustDialogAccepted` for the
> guest workdir the same way.
>
> So the step still runs with a **Linux/podman borrower** — that is where the
> whole lane is proven end to end — but the macOS residual is now narrower and
> honestly stated: **whether macOS `claude` prefers the login keychain over
> `<CLAUDE_CONFIG_DIR>/.credentials.json` is Claude Code's own behaviour and is
> not observable from this repo.** If the macOS interactive case fails, file it
> as that, with the observed broker trace (below), and do not attribute it to
> plumbing this tree does not contain.
>
> There is no "documented gap" arm left on the Linux borrower: it works or it is
> a finding.

> **[#844] the one debug line that settles a TUI auth failure in one grep.**
> `log_request` (`crates/services/broker/src/lib.rs`) is `axum` middleware added
> **last** = outermost on **both** broker routers, so even `DefaultBodyLimit`
> rejections and `fallback(reject)` 403s are logged. `debug`, target
> `ducktape::broker`, message `brokered request`, fields `method` / `path` /
> `status` and nothing else — the URI query is dropped by construction.
>
> **It is off unless you ask for it.** Every `RUST_LOG` in this runbook is
> `info,ducktape::service=debug` or `…gateway=debug`; none enables
> `ducktape::broker=debug`. The broker runs inside the daemon that spawned the
> run — **compute** for `sched`, **agent** for `pty` — so add it to that daemon's
> `RUST_LOG` before T2-5/T2-6 or the line will not be in `compute.log` /
> `agent.log`.
>
> The pre-#844 signature, for recognition: `method=HEAD path=/api/hello
> status=403` and **no `/v1/messages` line at all** across a whole session — the
> TUI never asked the broker anything about auth, it decided locally that it was
> unonboarded. `/api/hello → 403` occurs in the WORKING case too, so on its own
> it is not a fault; the absence of `/v1/messages` is.

Note T1-8's constraint applies to the borrower: `agent pty` reads the workspace
secret, so the box you run the CLI on must have read access to that node's
workspace.

- **PASS** = the console renders, the session is interactive, and the provider
  answers on the **lent** credential (not a local one — confirm the lender's
  `airlock.log` carries `airlock session opened credential=alice-claude-1
  caller=<8 hex> work=direct` at `info` for this session). **`work=direct` is
  the expected value here and `delegated(..)` would be the bug**: a pty has no
  committed record of who asked for it, so it sends `WorkRef::Direct` and draws
  on the executing node's own grant — the one T2-4 issued. This is the step that
  makes T2-4's grant load-bearing (§0.4a).
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

## 4. On/off isolation matrix (I) — 7 steps

**The claim:** separate processes mean separate failure domains. With all three
enabled, toggle each independently and prove the others are unaffected.

> **THIS SECTION IS NOT CREDENTIAL-FREE. It was scheduled as if it were, and it
> is not.** I-1, I-2 and I-3 each need work in flight, and there is no
> credential-free way to put work in flight:
> - **`agent sched` hard-requires `--cred <NAME>`** (`SchedArgs.cred: String`,
>   `bin/node/src/agent_cli.rs`) — I-2 and X-1 cannot be run without one;
> - `agent pty`'s `--cred` is optional, but a pty with **no** credential falls
>   through to `AnthropicAuth::from_host()` and reads the operator's live
>   `~/.claude/.credentials.json`, which **T1-6's HOLD forbids** — so I-1 needs
>   one too;
> - I-3 is a lent-credential step by definition.
>
> **So I-1, I-2 and I-3 are Tier-1-dependent: they run after T1-6 and T2-2, on
> the user-supplied throwaway credential, not before.** If the pass reaches this
> section without one, record all three as
> `BLOCKED(awaiting user-supplied throwaway credential)` — the same verdict as
> T1-6 — and run I-0, I-4 and I-5, which genuinely need nothing.
>
> **The substitution, if a credential-free smoke of the isolation property is
> wanted:** I-4 and I-5 already prove the blast-radius half with no work in
> flight at all (SIGKILL a daemon, watch the others). What they do **not** prove
> is the *in-flight survives a disable* half, and there is no credential-free
> stand-in for that — do not invent one, and do not record I-1/I-2/I-3 as PASS
> off I-4's evidence.

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
**Needs T1-6's credential** (see the section head). Start a pty session on it
(`"$D" agent pty claude --cred alice-claude-1 -n "$CHAIN"`, T1-8). With it live,
`service disable compute`.
**Pass:** the pty session **keeps running** — proved by typing into it and
seeing the echo *after* the disable returned, not by the absence of a
disconnect; and `service list` shows compute gone from the grants and agent
still `✓ enabled`.
**Fail:** the pty drops, or input stops echoing.

### I-2 — disable agent while a compute run is in flight
**Needs T1-6's credential.** Submit a long `agent sched --cred …` run, capture
its `$SAGA`, then `service disable agent`.
**Pass:** the run completes — `saga_get "$SAGA"` reaches `status: Done` with the
expected text in the decoded `result`.

> **`recent_runs` is the wrong read here, exactly as in T1-7.** This step used
> to say "delivers its result into `recent_runs`". It never will: `agent sched`
> triggers a saga with `reply_to: None`, and `RunsQuery::RecentRuns` projects
> the delivered-runs ring, which that trigger does not enter. Measured `[]` for
> every run placed on the first pass, including a committed `Done` carrying
> `PONG`. Use `saga_get`.

### I-3 — the airlock daemon goes away under a live session
**Needs T1-6's credential.** **Split into two cases, because the version this
step used to carry asserted both halves of a contradiction.**

> **What was wrong.** It asserted (a) a clean SIGTERM removes the route from
> `gateway-routes.json` *and* (b) a new session then gets
> `airlock_gateway_unreachable`. **(b) is false *because* (a) is true.**
> `proxy_loopback` (`bin/node/src/gateway_plane.rs`) looks the label up in
> `gateway-routes.json`; a removed entry is
> `GatewayFailure::NotFound("global gateway route has no local loopback
> upstream")` → **404** → the broker's `of_status(404)` → `Absent` →
> **`airlock_route_or_credential_absent`**. `airlock_gateway_unreachable` is
> `of_status(502)`, which needs a route that is still **present** with nothing
> listening — the **SIGKILL** case. The two cannot both hold on one teardown.

**I-3a — clean SIGTERM.** `stop_by_config "service run airlock"`.
**Pass, all three:**
- the in-flight session survives (`SESSION_TTL_SECS = 3600`,
  `MAX_REQUESTS = 4096` are per-session and fixed)
- the `airlock` entry is **gone** from `gateway-routes.json`, and if it was the
  only route the **file is removed**, not left as `{"routes":[]}`
- a **new** `agent sched --cred` run is refused
  **`airlock_route_or_credential_absent`** (404). Record `EXPECTED-REFUSAL`.

**I-3b — SIGKILL** (this is the `airlock_gateway_unreachable` case, and it is
the one I-4 also documents). Restart the airlock daemon, confirm the route is
back in `gateway-routes.json`, then `kill -KILL` it by verified identity.
**Pass:**
- the route is **still in `gateway-routes.json`** — nothing evicts it
- `ss -ltn "sport = :<N>"` shows nothing listening
- a new run is refused **`airlock_gateway_unreachable`** (502 — the node
  answered for a daemon it could not reach). Record `EXPECTED-REFUSAL`.

**Fail (either case):** the token from the *other* case. That is the one outcome
that says the 404/502 split is not wired the way the taxonomy claims.

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
CSOCK=$(own_sock compute) || exit 1     # §0.1 rule 4. The glob this step used
ASOCK=$(own_sock agent)   || exit 1     # to carry is how a shared box makes
                                        # this step pass against a STRANGER's
                                        # containers.
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

**Fail:** a container whose `io.ducktape.managed` value is **not** the display
id of the daemon that owns that socket — i.e. `agent#…` seen on the compute
socket, or a container with the label absent entirely.

> **The `unscoped` FAIL condition this step used to carry is DELETED — no
> production path can produce it.** `io.ducktape.managed=unscoped` comes from
> `UNSCOPED_OWNER` (`crates/services/provider/src/lib.rs`), which is set in
> exactly one place: `CliProvider::from_spec`'s struct literal. Every non-test
> caller of `from_spec` is inside `discover_with_sink`, which chains
> `.with_managed_owner(managed_owner)` on the very next line and overwrites it —
> `discover(…, managed_owner)` is the only way a daemon builds a provider set.
> The remaining `from_spec` callers are all `#[cfg(test)]`, and the ones that
> could reach a create use `SandboxBackend::Bare`, which never creates a
> container (`interactive.rs` reaches `unreachable!("interactive sessions never
> run under the bare test harness")`).
>
> An assertion nothing can trip is worse than no assertion: it reads as
> coverage of the "unreaped container" class and covers nothing. The real
> mechanism — **per-service graph roots**, so neither socket can enumerate the
> other's containers — is the third bullet above, and that one is observable.

### C-2 — one daemon's shutdown cannot kill the other's containers
Stop the compute daemon (SIGTERM via `stop_by_config`), while an agent session
holds a live container.
**Pass:** the agent's container is still `running` on the agent socket after the
compute daemon and its podman service are gone.
**Why it holds:** `PodmanService` supervises its child with `kill_on_drop`; a
*shared* service would die with whichever daemon started it and take the other's
containers along. That is exactly why per-service services were chosen over
labels.

### C-3 — crash-orphan sweep across restart
**[#841] All three strings this step used to assert on are gone from the tree.**
`reaped orphaned sandbox containers`, `reason="own_orphans"` and
`reason="reap_failed"` return zero hits at `02ad78b5e`. Compute and agent now
share one `sweep_own_containers` (`bin/node/src/services.rs`), used at BOTH ends
of a daemon's life, and it names the two ends differently on purpose.

**SIGKILL is now the only way to set this step up.** A clean stop sweeps its own
containers before stopping the sandbox service, so a SIGTERM leaves nothing for
the boot sweep to find. SIGKILL the compute daemon mid-run so a container is
left behind, then restart it.

**Wait on:** `await_line "$LOGS/compute.log" 'containers left by an earlier death were removed'`

**Pass:** that line is **WARN**, target `ducktape::service`,
`reason="crash_orphans_destroyed"`, and carries `instance=` and `removed=<n>`
(n ≥ 1) — and **the same `$CSOCK` container list is shorter by n afterwards**;
read it, do not infer it. This is the second reason instance ids must survive a
restart: the daemon recognises its own containers only if it returns with the
**same** id, which it does because the id is the grant hash and the grant
persists in `services.toml`.

**Fail:** `reason="container_sweep_failed"` (WARN, carries the error), or
**no line at all** — `removed == 0` yields `SweepReport::Quiet` and logs
**nothing** on either end, so silence here means the sweep found nothing, which
with a container still on the socket is the failure.

> **The clean-stop twin, for reference — it is not this step's line.** A
> SIGTERM'd daemon logs **INFO** `reason="own_containers_removed"`, *"this
> instance's containers were removed before its sandbox service stopped"*. If
> C-3 shows you that one, you did not SIGKILL.

> **The second FAIL clause — "or the count includes the agent's containers" — is
> DELETED as unreachable.** `reap()` (`bin/node/src/compute/mod.rs`) destructures
> the backend for its **own** `socket` and calls
> `reap_by_label(socket, managed_label(grant.display_id()))`: the sweep is scoped
> by **socket before the label is ever consulted**, and the agent daemon's
> containers live in a different graph root that is **not enumerable through the
> compute socket at all** (measured: 404). The function's own doc says so —
> *"Unreachable by construction, not pending cleanup."* Assert the label scoping
> in C-1, where it is observable, and assert the count here.
>
> **[§0.5 known-open] this step names a behaviour the design says should not
> happen, and #841 made the code SAY so instead of hiding it.** The design
> intent is re-**adoption**; there is no attach path in the tree, so the boot
> sweep destroys. It is a WARN now, and its own message states the consequence:
> *"…were removed, not resumed — their work re-executes from the start."*
> Record the line as evidence for the open "containers killed rather than
> re-adopted across restart" item, and do **not** also record R-4's "re-adopts
> its container" half as a fresh finding.

---

## 6. Cold start (K) — 2 steps

### K-1 — a genuinely cold node takes a run
**This step could not fail as written. The fix is P-4b's.**

Make the image store genuinely empty (do this **with the daemon stopped** —
`PodmanService::claim` treats two supervisors on one root as the hazard it
exists to prevent). **`rm -rf` does not work here**: this is a rootless overlay
graph root, UID-mapped into a user namespace, so `rm -rf` **exits 1 having
removed ~98% of the bytes** and left a small residue with the directory
standing (P-4b has the measured numbers) — enough for the daemon to come back
onto a store that is not empty, so the "cold" run may be warm, no pull happens,
and the step passes having tested nothing.

```bash
stop_by_config "service run compute"
podman unshare rm -rf "$WS/storage/services/compute/podman/storage"
```

**Prove the store is actually empty before starting** — this assertion is the
step, and per P-4b **`du` is trustworthy here**: it reports the same size inside
and outside the user namespace.
```bash
du -sh "$WS/storage/services/compute/podman/storage" 2>/dev/null   # gone, or ~0
```
Restart the daemon, then:
```bash
CSOCK=$(own_sock compute) || exit 1                                 # §0.1 rule 4
curl -s --unix-socket "$CSOCK" http://d/v5.0.0/libpod/images/json   # MUST be []
```
**If the image list is non-empty, the reap did not work and K-1 has not
started.** Then submit one run (T1-7) — **which needs T1-6's credential and a
running airlock daemon** (T1-7's head note); K-1 is not credential-free either.

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

> **CORRECTION: the pull is NOT invisible, and its two lines are this step's
> best observable.** The old note said `podman_api.rs` contains *"zero
> `tracing::` calls"*. It contains four, and three agents on the first pass
> watched the pair fire, **~5–6 s per graph root** for `node:22-slim`:
> ```
> INFO ducktape::sandbox: pulling provider image into this service's store image=docker.io/library/node:22-slim
> INFO ducktape::sandbox: provider image pulled image=docker.io/library/node:22-slim
> ```
> **Use them as the step's wait, not a stopwatch:**
> ```bash
> await_line "$LOGS/compute.log" 'pulling provider image into this service.s store'
> await_line "$LOGS/compute.log" 'provider image pulled'
> ```
> They are `INFO` on `ducktape::sandbox`, so `RUST_LOG=info` is enough. Seeing
> the **first** without the **second** is a genuinely stuck pull; seeing neither
> means the store was not cold and the `podman unshare` reap above did not take.
> Budget seconds, not "several minutes" — and if it does run long, it is a
> network fact to report with a number, not a silence to excuse.

> **A trap in the pull path itself:** libpod's pull endpoint returns **HTTP 200
> on a *failed* pull** — the verdict is an `{"error":…}` line inside the
> streamed body (`pull_failure`, `podman_api.rs`). If a run fails right
> after a cold start with no obvious cause, that is where to look.

### K-2 — the claimed residual: does a cold winner lose its lease to its own download?

The claimed residual is that the pull happens at first run *inside* the lease
window, so a cold winner can lose its lease to its own image download.

**The arithmetic does not support that story, and this step exists to settle
it.** An agent run's lease is `RUN_LEASE_VIEWS = 1024` views
(`crates/modules/apps/runs/src/lib.rs`); the host heartbeat fires every **10 s**
(`compute/pool.rs`) and is `select`ed against the run future, so it covers the
create/pull; and each renewal past the half-window resets expiry to
`height + 1024`. The measured pull is **~5–6 s** (K-1). Nothing comes close.

> **CORRECTION: the window is ~8.5 minutes, not ~17.** The old figure multiplied
> 1024 views by `BLOCK_TIME = 1 s`. `BLOCK_TIME` is the **idle** heartbeat only
> — the constant's own doc says a busy chain *"has NO interval knob at all"* and
> its rate is set by the network's agreement speed
> (`crates/kernel/consensus/src/lib.rs`). **Measured on this chain: `height 2022
> → 2042 over 10 s` — 2 blocks/s.** So 1024 views ≈ **8.5 min**, and the
> conclusion is unchanged (a 6 s pull does not outlast 8.5 min) but **R-4's
> "leave it down long enough for the lease to lapse" is a ~8.5 min wait, not
> ~17.** Measure the rate on the day rather than trusting either number:
> ```bash
> h1=$("$D" node status -n "$CHAIN" --json | python3 -c 'import sys,json;print(json.load(sys.stdin)["height"])')
> read -r -t 10 </dev/zero; \
> h2=$("$D" node status -n "$CHAIN" --json | python3 -c 'import sys,json;print(json.load(sys.stdin)["height"])')
> echo "blocks/s = $(( (h2 - h1) / 10 ));  lease window ≈ $(( 1024 * 10 / (h2 - h1) ))s"
> ```

> Do not confuse it with `JOB_RUN_LEASE_VIEWS = 1000` in the same file — a
> different constant on the jobs lane.

**So the expected result is: the cold run completes and the lease is never
lost.**

**How to tell a pass from a failure:**
- **PASS:** run completes; `grep -c 'lease_renew_failed' "$LOGS/compute.log"` = 0;
  the saga shows **one** attempt.
- **RESIDUAL CONFIRMED (report loudly):** `saga_get "$SAGA"` shows `attempt=1`
  or higher, **or** `lease_renew_failed` appears. Then capture
  `RUST_LOG=ducktape::saga=debug` output — the cause is either the `RenewLease`
  origin check refusing or the renew submit failing.
  (**Not `recent_runs`** — it is `[]` for every `sched` saga; see T1-7's box.)
- **NOTE:** on lease expiry the attempt is **cancelled and re-placed**, not
  dropped: `lease_and_request` recomputes the assignee by rendezvous over
  `(saga_id, attempt, height)`. `RUN_MAX_ATTEMPTS = 2`, so a *second* expiry
  fails the saga with `lease attempts exhausted`. Nothing is silently lost.
- **There is no "lease lost" or "run re-placed" log line at all** — `saga` is a
  consensus module and emits no `tracing`. The only observable is committed
  state via `SagaQuery` `get`. (**`runs` queries are not an alternative here** —
  a `sched` saga never enters either `runs` projection; T1-7's box.)

---

## 7. Restart and skew (R) — 4 steps

### R-1 — daemon restart keeps the instance id
**Its stated observable could not fail. Replaced.**

> **What was wrong.** It said *"`service status` shows the same
> `compute#<hex8>`"*. `service status` renders the id out of the **grant file**,
> and a daemon restart never touches `services.toml` — so that read returns the
> same string whether or not the daemon came back, whether or not it came back
> with the right id, and even if it never started. It is a read of the input,
> presented as a read of the output.
>
> **The falsifiable observable is the daemon's own boot line.** `compute::serve`
> logs `instance = %grant.display_id()` on `compute daemon serving`
> (`bin/node/src/compute/mod.rs`) — a value the **restarted process** computed,
> after re-reading and re-hashing the grant. That is what "the id survived a
> restart" actually means.

Stop and restart the compute daemon.
```bash
before=$(grep -o 'instance=compute#[0-9a-f]*' "$LOGS/compute.log" | tail -1)
stop_by_config "service run compute"
RUST_LOG=info,ducktape::service=debug,ducktape::saga=debug \
  "$D" service run compute -n "$CHAIN" > "$LOGS/compute.out" 2>> "$LOGS/compute.log" &
await_line "$LOGS/compute.log" 'compute daemon serving'
after=$(grep -o 'instance=compute#[0-9a-f]*' "$LOGS/compute.log" | tail -1)
echo "$before -> $after";  test "$before" = "$after"
```
**Pass:** two `compute daemon serving` lines from two different processes
carrying the **same** `instance=`; `services.toml` unchanged (compare a hash).

**Then** `service disable compute` + `service enable compute`, restart the
daemon, and assert the **new** `compute daemon serving` line carries a
**different** `instance=` — a re-enable mints a fresh nonce and therefore a
fresh id. That asymmetry is the consent-epoch property; both halves must hold.
**[#829] both halves also report a height** on the verb's stderr. Record them; a
re-enable that returns the same id *or* the same height breaks the property.

**Also record [§0.5 known-open]:** whether the old daemon's `podman system
service` is still alive after `stop_by_config` returned
(`pgrep -u "$USER" -x podman | wc -l` before and after). It is, today — that is
the "daemon orphans its podman service on every stop path" bug, and it is why
the restarted daemon can find a socket it did not create.

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
  now signals `build = "unknown"` and serves.

  > **The method this step used to suggest — "with `.git` hidden" — silently
  > produces a WRONG stamp, and a literal follower tests nothing.** `bin/noded/
  > build.rs` shells out to `git`, and `git` **walks up**. Hiding a worktree's
  > `.git` *file* does not make git absent: it finds the enclosing primary
  > checkout and stamps **its** HEAD. You get a plausible-looking commit hash
  > instead of `unknown`, the daemon signals a real build, and the step passes
  > having exercised nothing. And `CLAUDE.md` **mandates** worktrees nested
  > under the primary checkout (`<primary>/.worktree/<slug>`), so this is the
  > normal case here, not an edge one.
  >
  > **Two methods that actually work:**
  > ```bash
  > # (a) a failing `git` shim, first on PATH — build.rs's git invocation fails,
  > #     DUCKTAPE_BUILD is never emitted, build_identity() is None.
  > mkdir -p "$TMPDIR/nogit" && printf '#!/bin/sh\nexit 127\n' > "$TMPDIR/nogit/git"
  > chmod +x "$TMPDIR/nogit/git"
  > PATH="$TMPDIR/nogit:$PATH" cargo build --release -p node-bin --bin ducktape
  >
  > # (b) a real tarball outside any checkout — no .git anywhere up the tree.
  > git archive --format=tar HEAD | (mkdir -p "$TMPDIR/tarball" && tar -x -C "$TMPDIR/tarball")
  > ```
  > `$TMPDIR` is disk-backed and outside every worktree (§0.3), which method (b)
  > requires and method (a) merely benefits from.
  >
  > **Confirm it before trusting it:** the daemon's hello must carry
  > `build = "unknown"`, and `Skew::Unknown` **warns about nothing** — so the
  > pass here is *`service run` starts and serves*, plus **zero**
  > `reason="build_skew"` lines. A `build_skew` warn from this binary means git
  > was not actually absent and you stamped the enclosing checkout.
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
**Needs T1-6's credential** (`agent sched` requires `--cred`; §4's head note).

Kill the compute daemon mid-run and restart it promptly.
**Pass:** the saga stays on **attempt 0** (the lease is renewed from the restored
heartbeat), read with `saga_get "$SAGA"` — **`runs` queries do not see a `sched`
saga at all** (T1-7's box), so `saga` `get` is the only read here, not
"`runs`/`saga` queries".

> **[§0.5 known-open] the "re-adopts its container" half will not hold today.**
> The restarted daemon **reaps** its orphans rather than re-adopting them (C-3).
> Record the reap, cite the open bug, and do **not** file it twice.

Then repeat, leaving it down long enough for the lease to lapse: the attempt is
cancelled and re-placed (`attempt` increments). With `RUN_MAX_ATTEMPTS = 2`, a
second lapse fails the saga with `lease attempts exhausted`.

**How long is "long enough": ~8.5 min on this chain, not the ~17 the old
arithmetic implied** — `RUN_LEASE_VIEWS = 1024` at a **measured 2 blocks/s**
(K-2 carries the one-liner that measures it; measure on the day). This is the
longest single wait in the runbook — start it, and do something else.
**Assert via `saga` `get` — there is no log line.**

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
> **silent park**: no error, no timeout, `saga_get` stuck on `Pending` with
> `assignee=False`, forever. Run the negative FIRST so the difference is
> observable, and never read "it is still going" as slow.
> (`recent_runs` is not the read here or anywhere in this runbook — T1-7's box.)

**X-1a — the negative, before `node work admit`.** Disable compute on the dev
box; leave it enabled and serving on the borrower. Submit an unpinned run from
the dev box.
**Pass:** the borrower's `compute.log` carries exactly one WARN, target
`ducktape::saga`, `reason="work_not_admitted"`, message **`compute claim
refused`** (the claim lane's message, distinct from T2-5's `compute attempt
refused`) for that saga — once, because the decision is latched — the saga stays
`Pending` with `assignee: null`, and **no container is created**:
```bash
saga_get "$SAGA"                                   # T1-7's helper
CSOCK=$(own_sock compute) || exit 1                # on the BORROWER — §0.1 rule 4
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

> **THE PINNED HASH IS NOT THE HASH ANY REAL NODE LOGS. Do not compare them.**
> `GENESIS_ROOT_HASH` is composed over `PIN_BINDINGS` — literally
> `{ invite: b"parity-test", identity_chain_id: "parity-test" }` — and the
> constant's own doc explains why: each binding rides its module's store as a
> genesis `__config` record, so *"a real network's invite namespace and chain id
> put it on its own root **by design**"*. A live `w2qa#…` node composes over its
> **own** chain id and invite namespace and will log a different 64 hex
> characters. An operator who diffs `daemon.log`'s `genesis root_hash=` against
> the constant will conclude consensus moved. **It did not.** The two answer
> different questions: the test says "this tree's genesis composition is
> unmoved"; the cross-box comparison says "these two nodes agree with each
> other".
>
> **The cross-box half stays manual, and it has a trap of its own.**
> ```bash
> grep 'genesis root_hash=' "$WS/daemon.log" | tail -1     # on BOTH boxes
> "$D" node status -n "$CHAIN"                             # on BOTH boxes
> ```
> assert the two are byte-identical **to each other**, never to the constant.
> But `genesis root_hash=` is logged **once, at genesis, and only on a fresh
> boot**: `bin/node/src/validator/engine.rs` emits it under
> `if resumed.is_none()`, and a restored boot prints its recovered line instead.
> So on a **restarted** node, a **rotated** `daemon.log`, or a **state-synced
> joiner**, that grep finds nothing — which is not a mismatch and is not
> evidence of anything.
>
> **Capture it at T1-1, on each box's first boot, and carry the value forward.**
> If you reach V-1 with an empty grep, say `NOT-CAPTURED(restarted/synced)` —
> do **not** restart a node to make the line reappear, because a fresh genesis
> is exactly what a restart does not produce.
>
> Drop the old "compare against T1-1" half of the *tree* pin — the test covers
> it without a human transcribing 64 hex characters.

> Do not confuse it with the sim pin. `DEFAULT_GENESIS_ROOT_HASH` in
> `bin/simnode/tests/topology_set.rs` is a **different** hash over a
> **different** 14-module set (it excludes `capability`, `hello`, `governance`,
> `lifecycle`) and its own doc disclaims consensus relevance. Running
> `cargo test -p simnode --test topology_set` is a fine sanity check and is
> **not** the consensus pin.

### V-2 — `/v1` additive only
**Re-anchored TWICE. Both earlier forms could not fail, for different reasons.**

> **Form 1 — `git diff origin/dev...HEAD -- bin/noded/src/lib.rs | grep '^-.*\.route('`.**
> Empty by construction: the pass runs *on* dev.
>
> **Form 2 — baseline the route list at P-2, diff it at V-2.** Also empty by
> construction, and this one *looked* like a real check. Both reads come from
> **the same immutable checkout**, and P-2 asserts `git status --porcelain` is
> empty, so the two files are identical no matter what the router does.
> **Proven:** deleting the entire multi-line `/v1/files/stage` route block from a
> scratch copy left the count at **33** and the diff **empty**.
>
> **Form 2's extractor was also blind to 11 of the 44 routes — 25%.**
> `grep -oE '\.route\("(/v1[^"]*)"'` matches only when the path literal shares a
> line with `.route(`, and it read only `lib.rs`. It missed:
> - **six multi-line routes in `lib.rs`** — `/v1/gateway/proxy`,
>   `/v1/files/blob`, `/v1/files/stage`, `/v1/files/object/{*path}`,
>   `/v1/fs/workspaces/{id}/commit`, `/v1/fs/workspaces/{id}` (each wrapped
>   because it carries a `DefaultBodyLimit` layer);
> - **all five `/v1/admin/*` routes**, which live in `bin/noded/src/admin.rs` and
>   are `merge`d into the same public router.
>
> A route-removal check that cannot see a quarter of the routes, comparing a
> file against itself, is the exact shape §0.1 rule 3 forbids.

**The baseline is the committed list below** — this document is checked in, so
the list is a real prior artifact rather than a second read of the tree under
test. The extractor is multi-line-aware and reads **both** router files.

```bash
# at P-2 (already run) and again at V-2:
v1_routes() {
  cat bin/noded/src/lib.rs bin/noded/src/admin.rs \
    | tr '\n' ' ' | grep -oE '\.route\( *"/v1[^"]*"' \
    | sed 's/.*"\(.*\)"/\1/' | sort -u
}
v1_routes > "$LOGS/routes.now"          # NOT /tmp — §0.3
wc -l < "$LOGS/routes.now"              # 44

# save the block below as $LOGS/routes.baseline, then:
diff "$LOGS/routes.baseline" "$LOGS/routes.now"
```

**The committed baseline — 44 routes, `dev` @ `feec0a6db`:**

```
/v1/admin/logs/tail
/v1/admin/module-code/stage
/v1/admin/module-code/{digest}
/v1/admin/ping
/v1/admin/shutdown
/v1/blocks
/v1/call/ws
/v1/files/blob
/v1/files/blob/{digest}
/v1/files/commit
/v1/files/diff
/v1/files/find
/v1/files/grep
/v1/files/has-chunks
/v1/files/history
/v1/files/ls
/v1/files/object/{*path}
/v1/files/pin
/v1/files/read
/v1/files/refs
/v1/files/stage
/v1/files/stat
/v1/files/watch
/v1/fs/workspaces
/v1/fs/workspaces/{id}
/v1/fs/workspaces/{id}/commit
/v1/gateway/browser
/v1/gateway/proxy
/v1/index/status
/v1/index/{module}/ops
/v1/index/{module}/scan
/v1/index/{module}/view
/v1/log-filter
/v1/peers
/v1/presence/ws
/v1/query
/v1/services
/v1/services/hello
/v1/status
/v1/submit
/v1/submit/frame
/v1/term/sessions
/v1/term/sessions/{id}/close
/v1/ws
```

**Pass:** `diff` is empty, **or** contains only `>` lines (additions). New routes
are fine; a changed or removed one is a wire break.
**Fail:** any `<` line — a route in the committed baseline and gone from the
tree. **Also fail** on a count that is not 44 with an empty diff, which would
mean the extractor itself broke.

**Self-check the check, once, before trusting it** — this is the cheapest thing
in the runbook and it is what caught form 2:
```bash
cp bin/noded/src/lib.rs "$TMPDIR/lib.rs.bak"
python3 - <<'PY'
import re,pathlib
p = pathlib.Path("bin/noded/src/lib.rs"); s = p.read_text()
p.write_text(s.replace('"/v1/files/stage"', '"/v1/files/DELETED-CANARY"', 1))
PY
diff "$LOGS/routes.baseline" <(v1_routes)   # MUST show `< /v1/files/stage`
cp "$TMPDIR/lib.rs.bak" bin/noded/src/lib.rs
git diff --quiet bin/noded/src/lib.rs && echo "restored"
```
If that diff is empty, the extractor is broken and V-2 is worthless — fix the
extractor before reading its verdict.

This also gives the report a concrete number — **the `/v1` surface is 44 routes,
39 public plus 5 under `/v1/admin/*`** — instead of an unfalsifiable "additive
only". Note the admin five are only *mounted* when `exposure.enabled()`
(V-4); they are in the source inventory either way.

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
# NOTE the [0-9a-f]{64} arm. Without it this grep cannot see either secret
# this runbook actually mints — see the box below.
grep -rniE 'admin\.token|service-link|sk-|Bearer |accessToken|refreshToken|[0-9a-f]{64}' \
     "$WS/daemon.log" "$LOGS"/*.log | grep -v 'admin.token in the node' | head -40
```

> **The old pattern was blind to both secrets this pass creates.** `admin.token`
> and `service-link.token` are **bare 64-lowercase-hex with no prefix** — no
> `sk-`, no `Bearer `, no JSON key. Every alternative in the old pattern matched
> only the **filename**, never the value. **Proven:** a synthetic log line
> carrying a real 64-hex token was invisible to it. `[0-9a-f]{64}` is what
> closes it.
>
> **It is a noisy arm, deliberately, and the noise is the work.** 64 hex also
> matches root hashes, digests, saga ids and node keys — all of which are
> legitimately logged. So this arm is a **review list, not a boolean**: read
> every hit and classify it. To make that tractable, diff the hits against the
> two values you already hold, without ever printing them:
> ```bash
> # exit 1 if either minted secret appears verbatim anywhere. Prints NO secret.
> python3 - "$WS" "$LOGS" <<'PY'
> import pathlib,sys,glob
> ws, logs = pathlib.Path(sys.argv[1]), sys.argv[2]
> secrets = {n: (ws/n).read_text().strip() for n in ("admin.token","service-link.token")
>            if (ws/n).exists()}
> bad = 0
> for f in [str(ws/"daemon.log"), *glob.glob(logs+"/*.log")]:
>     try: text = pathlib.Path(f).read_text(errors="replace")
>     except OSError: continue
>     for name, value in secrets.items():
>         if value and value in text:
>             print(f"LEAK: {name} appears verbatim in {f}"); bad = 1
> print("clean" if not bad else "LEAKED"); sys.exit(bad)
> PY
> ```
> That is the falsifiable half; the `head -40` above is the exploratory half.
> **Run both. Neither replaces the other**, and note the pass must re-run the
> exact-value check **after every node restart** (R-2 mints fresh ones, so a
> check run only at the end tests only the last pair).

**Pass:** the exact-value check exits 0, **and** every `[0-9a-f]{64}` hit is
classified as a hash/id in the report. A credential **name** legitimately
appears (`credential=<name>` on the airlock grant gate) — that is by design; the
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
| with `DUCKTAPE_ADMIN=off` | **404** | **none — assert an EMPTY body** |

> **Row 5's `admin_namespace_absent` is unreachable on `ducktape node run`.**
> `AdminExposure::Disabled` does map to `AdminRefusal::NamespaceAbsent` in
> `admit_gate` — but under `DUCKTAPE_ADMIN=off` **that gate never runs**, because
> `bin/noded/src/lib.rs` merges `admin::admin_router` only
> `if handle.admin.exposure.enabled()`. The routes are never registered, so
> axum's plain fallback answers: a **bare 404 with an empty body and no
> `reason` field at all**. The comment at the merge says exactly this —
> *"`Disabled` leaves the control surface simply ABSENT (a 404), not a
> gated-but-present route."* The token is reachable only from a unit test.
>
> **Assert the 404 AND the empty body**, which is the observable difference
> between "absent" and "present but refusing":
> ```bash
> DUCKTAPE_ADMIN=off  # on the NODE's environment, then restart it
> curl -s -o "$TMPDIR/body" -w '%{http_code} %{size_download}\n' \
>   "http://127.0.0.1:$HTTP/v1/admin/ping"      # -> `404 0`
> ```
> **Fail:** a 404 with a JSON body naming `admin_namespace_absent` — that would
> mean the router is being mounted and then refusing, which is not the design.

Body shape for every **other** row is `{"error":…,"reason":…}`. **Node-side the
refusal is logged at `debug`**, target `ducktape::admin` — set
`RUST_LOG=ducktape::admin=debug` or you will see nothing.
On an **owned** node (post-T1-6) the same requests yield `not_the_owner` /
`owner_signature_invalid` instead; assert that too, and that
`ducktape user sign-admin` produces headers the node accepts.

### V-5 — no daemon holds the node private key
**The fd probe could not fail. The structural test is now the pass criterion.**

> **Why the fd probe proves nothing.** It counted `identity.key` in
> `/proc/<pid>/fd` and expected `0` for each daemon. Measured on the first pass:
> **`identity.key_fds` is `0` for the NODE ITSELF** — the process that
> unambiguously does read the key. It opens the file, reads it, and **closes**
> it, so an instantaneous fd scan sees nothing. **A daemon that stole the key
> exactly as the node does would score 0 and pass.** The probe measures "is a
> file handle open at this microsecond", which is not the property.

**The pass criterion is the source-parsing lint test**, which is green on this
tree and cannot be satisfied by timing:

```bash
cargo test -p node-bin --bin ducktape the_daemon_path_cannot_name_the_node_key
```

It parses `bin/node/src/services.rs` and fails the build if the daemon path
*mentions* the key at all — a static property, unaffected by when anything is
open. Alongside it: `ServiceConfig` has no field a secret could live in,
`resolve_service` never opens `identity.key`, and containment runs one way
(`Resolved` holds a `ServiceConfig`, never the reverse).

**The fd probe is demoted to a note, not deleted** — it is still worth running
because a **non-zero** count would be a real finding (a daemon holding the file
open is unambiguously wrong). A **zero** is simply not evidence:
```bash
for p in $(pgrep -u "$USER" -x ducktape); do
  tr '\0' ' ' < "/proc/$p/cmdline" | grep -q 'service run' || continue
  printf 'pid %s identity.key fds: %s\n' "$p" \
    "$(ls -l "/proc/$p/fd" 2>/dev/null | grep -c 'identity.key')"
done
```
Report it as `informational (0 is not evidence — the node scores 0 too)`.

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
| `credential_not_granted` | HTTP 403 with that body token. The lender's grant gate consulted its committed record and refused. **[#833] this no longer fires for a missing account** — see below. **[#843] it is now SEVEN different node-side decisions wearing one body token** — the caller's own grant plus the five delegated conditions plus an absent record; the lender's own log names which (see the lender-side table). Do not report a cause you did not read there. |
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

**Node-side REFUSAL** (`bin/node/src/airlock.rs`, `refuse()`, DEBUG, target
`ducktape::gateway`, message `airlock session refused`). **[#843, #847] It
carries `reason` and NOTHING else** — no credential, no account, no saga id.
That is deliberate and pinned by
`a_refusal_still_names_neither_the_credential_nor_the_caller`: a refusal is
reachable by any admitted member with a `sub` of their own choosing, so naming
the credential there would write a stranger's string into the owner's log. **A
step that greps a refusal line for `credential=` will find nothing** — that
field is on the ADMISSION line only.

| `reason` | trigger | resolves to |
|---|---|---|
| `credential_record_absent` | query OK, gateway module returned no record for that name | `Refused` |
| `grant_authority_unavailable` | the `/v1/query` itself failed — **note: no `airlock_` prefix on this side** | `Undetermined` |
| `credential_not_granted` | record exists; the vouched account is neither owner nor grantee **and the session presented `WorkRef::Direct`** (every interactive pty) | `Refused` |
| **`work_pointer_oversized`** | **[#843]** a `Saga` pointer over `MAX_WORK_POINTER_BYTES` = 512. A refusal, not an impossibility — no product path emits one. | `Refused` |
| **`delegated_work_unseen`** | **[#843]** this lender has not committed that saga: a follower behind head, or an id naming nothing. **Not a refusal** — the borrower re-asks (§0.4a). | `Undetermined` |
| **`delegated_work_finished`** | **[#843]** the saga is terminal. A finished run is not a standing licence. | `Refused` |
| **`delegated_work_names_another_credential`** | **[#843]** the committed spec names a different credential than the session's `sub`. The pointer buys one credential. | `Refused` |
| **`delegated_caller_not_the_executor`** | **[#843]** the vouched caller is not the saga's `pinned_assignee` — including an UNPINNED saga, which delegates to nobody. | `Refused` |
| **`delegated_submitter_not_granted`** | **[#843]** the saga's origin maps to an account the credential does not admit, or is `Module(_)`/`System`, which names no account to draw as. | `Refused` |

**Node-side ADMISSION** (`bin/node/src/airlock.rs`, `admit()`, **INFO**, target
`ducktape::gateway`, message `airlock session opened`). **[#847]** The owner's
own audit record, and the only line on this path visible at the default filter:

```
airlock session opened credential=<name> caller=<4-byte hex prefix> work=direct
airlock session opened credential=<name> caller=<4-byte hex prefix> work=delegated("sched\u{1f}<id>")
```

`work=` is a `Draw` discriminant, not a copy of the request: a session that
carried a pointer but was admitted on the caller's OWN grant records `direct`,
because the pointer bought nothing there. Never the token, the credential's
value, or the caller's whole account.

**Gateway-side HTTP** (`crates/airlock/src/server.rs`) — the plain body tokens a
borrower actually sees:

| body | status |
|---|---|
| `credential_not_found` | **404** |
| `caller_account_unverified` | **403** |
| `credential_not_granted` | **403** |
| `grant_authority_unavailable` | **503** |
| (decode failure — unknown/missing field) | **422** |

**Every `Refused` arm above collapses into `403 credential_not_granted` on the
wire, and every `Undetermined` arm into `503 grant_authority_unavailable`.** The
node-side tokens exist only in the lender's own log, at `debug` — invisible at
the default filter. A negative half that needs to tell the seven refusals apart
must start the lender daemon with `RUST_LOG=ducktape::gateway=debug`
(`RUST_LOG` **adds to** the default rather than replacing it,
`bin/noded/src/log.rs`). A report that lists a node-side token as an HTTP reason
is reading the wrong layer.

### Borrower side — the broker's own per-request line (`crates/services/broker/src/lib.rs`)

**[#844] `log_request`, DEBUG, target `ducktape::broker`, message
`brokered request`, fields `method` / `path` / `status`.** Not a refusal table —
it is the trace that tells a credential failure apart from a provider that never
asked. Outermost middleware on both routers, so body-limit rejections and
`fallback(reject)` 403s appear too. **Requires `ducktape::broker=debug` on the
daemon that spawns the run** (compute for `sched`, agent for `pty`); no
`RUST_LOG` in this runbook enables it by default.

### Provider run home (`crates/services/provider/src/lib.rs`)

| `reason` | level | meaning |
|---|---|---|
| **`config_home_not_removed`** | WARN, target `ducktape::provider`, `"the run's config home outlived its run"` | **[#845]** `RunHome`'s `Drop` could not remove `<workdir>/.ducktape-run/<slot>`. The path is deliberately absent from the line. Only a SIGKILLed host should leave one standing — `runtime_slot()` is 12 random bytes per run now, so no later run can name an inherited one. |

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
  the fastest way to read `capability all`. **But its `runs pending_runs` /
  `recent_runs` reads are the wrong lane for an `agent sched` run** — both are
  `[]` for every `sched` saga (T1-7's box). Use `saga get` on
  `"sched\x1f<dispatch>"`. Worth a line in that script's own help.
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
- **`docs/superpowers/plans/2026-07-26-work-admission.md` — two stale places,
  both corrected in the same PR as this revision.** (1) §7.1 described the
  shared-pty fix as a `MembersOnly` post policy with a participants roster; #835
  **rejected** that and gated at `project_message` on `Channel.owner` instead
  (§0.4b). (2) §4 "Delegation — after, not with" and decision 8.3 "Delegation
  ships next, not with" were **written before #843 and now describe shipped
  work**; §4 is no longer the reference for §0.4a — the code is, and §0.4a cites
  it directly. §5.5 also named a test that no longer exists under that name.
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
- **§0.1 rule 6's environment block** — SET/unset for every variable, before the
  first step
- the `/v1` route count from V-2 (**44**), the baseline diff, and whether the
  canary self-check showed `< /v1/files/stage`
- both boxes' genesis root hash **compared to each other only** (V-1), and the
  result of `production_genesis_root_hash_is_pinned` — **never the two compared
  to one another**
- the warm-restart publish→committed-height gap measured in T1-1 (the
  pre-publish window is closed; §0.1)
- the measured **blocks/s** and the derived lease window (K-2's one-liner)
- `df -h /tmp` before and after, and the P-4b `du` figures (before/after,
  **not** exit codes)
- every step that was BLOCKED or EXPECTED-REFUSAL, and why
- **which §0.5 known-open bugs this pass observed**, and where — they are not
  new findings
- **whether the credential was the user-supplied throwaway or something
  else.** Anything imported through `DUCKTAPE_CRED_REUSE_ARTIFACT` without the
  user's artifact is unverified and every credential-dependent verdict inherits
  that (§0.1 rule 6, T1-7's caveat).
- **the airlock-dependency finding, re-verified or not** (T1-7): whether an
  `agent sched --cred` run reaches the provider with **no** airlock daemon
  running. The first pass says it does not; that observation was made with a
  fabricated credential and needs one more run.
- **the honest list of what this did not prove** — copy §0.4 verbatim, including:
  - `/v1` is trusted-local; `origin_guard` passes every `Origin`-less caller
  - `grant.scopes` gates nothing
  - a same-uid process can read `identity.key`
  - the `logs` and `module:<id>` ws topics are Public
  - **delegation IS implemented** (#843/#847) and §0.4a says what it does and
    does not authorize. What this pass proves about it is one live cross-box
    round trip and the refusals it exercises — **not** the gate's own conjunction;
    the four conditions are proven by `cred_lending` and `sched_pinned_run`, and
    a green T2-5 alone is not evidence any one of them holds
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
| 4. On/off isolation (I) | 7 (I-3 split into I-3a/I-3b) |
| 5. Podman co-tenancy (C) | 3 |
| 6. Cold start (K) | 2 |
| 7. Restart and skew (R) | 4 |
| 8. Cross-node placement (X) | 3 (X-1 has two halves) |
| 9. Invariants (V) | 5 |
| **total** | **48** |

Fourteen carry a **deliberate-failure half** that must be run first: T1-3, T1-6
(P-6's flat-spelling half), T1-7, T1-8, T2-4, T2-5, T2-6, I-0, I-3a, I-3b, K-1,
R-3, X-1a, X-2, plus P-9's `DUCKTAPE_ALLOW_MISSING_TOOLS` check and V-2's
canary. **A pass that skipped the negatives has not established that any of the
positives can fail** — and the first execution is the proof: eight of this
document's positives could not fail at all until it ran.

### What the repair changed, by verdict

Read this before comparing a report against an older run of the same step ids.

**Steps whose pass criterion changed — an old PASS does not carry over:**
P-3, P-4 (a+b), P-6, P-9, T1-2, T1-7, I-2, I-3 (now I-3a/I-3b), C-3, K-1, K-2,
R-1, R-3, R-4, V-1, V-2, V-3, V-4, V-5.

**Steps whose pass criterion changed AGAIN in the 2026-07-27 re-repair (#843,
#847), and this set is sharper: an old EXPECTED-REFUSAL is now a FAIL.**

- **T2-4** — its deliberate-failure half was the exact inverse of the shipped
  behaviour. Old text: grant the lender's own account, run T2-5, expect
  `credential_not_granted`. That shape now returns `Done` + `PONG`. Replaced
  with a borrower-submits-pinned-to-itself refusal, which is a real gate.
- **T2-5** — its lender-log observable moved from `debug`/"never the account" to
  the `info` `airlock session opened` record, and gained the `work=delegated(..)`
  assertion that is the only thing distinguishing a delegated draw from a
  standing-grant one. It also gained the note that one 503 on this lane is
  expected weather, not a finding.
- **T2-6** — gained the mirrored `work=direct` assertion, and the statement that
  T2-4's grant is what this step needs (a pty sends `WorkRef::Direct` and cannot
  delegate).
- **T1-7** — its `grep -c 'credential='` was unsatisfiable by construction until
  #847 and is replaced with `airlock session opened` / `work=direct`.
- **§0.4a, §10's lender-side table, §12's honest list** — rewritten.

**And by the other four merges in the same window:**

- **C-3** (#841) — all three strings it asserted on are gone from the tree, and
  a clean stop now sweeps, so only a SIGKILL sets the step up at all.
- **P-3b** (#841, #846) — `--json` is an array and carries no node build;
  `service status <KIND>` parses now; the exe-path caveat is dead (`flock`).
- **T1-4** (#841) — the singleton guard DOES fire between two binary paths now,
  which is the opposite of what §0.5 said.
- **T2-5's flag-trap box** (#846) — `--node duke` is refused where it was typed;
  the reqwest `builder error` it told you to expect no longer happens.
- **T2-6** (#844) — the "Why not the macmini" box named two env vars and a
  commit that are not in this tree.
- **§0.5** (#841) — three of five fixed, one narrowed, one unchanged.

**Assertions deleted as unreachable, and why** (each is documented at its step —
an assertion nothing can trip reads as coverage and is worse than none):
- **C-1's `io.ducktape.managed=unscoped` FAIL** — `UNSCOPED_OWNER` is
  overwritten on the next line by `discover_with_sink`; every other producer is
  `#[cfg(test)]` on `SandboxBackend::Bare`, which never creates a container.
- **C-3's "the count includes the agent's containers" FAIL** — the reap is
  scoped by socket before the label is read, and the agent's containers are not
  enumerable through the compute socket (measured: 404).
- **V-4 row 5's `admin_namespace_absent` reason** — the admin router is not
  merged under `DUCKTAPE_ADMIN=off`, so the gate never runs; replaced with the
  bare-404-plus-empty-body assertion.
- **P-9's `grep -c 'compute daemon serving' "$LOGS"/*.log`** — no daemon exists
  in `$LOGS` at precondition time and `dispatch_e2e` logs elsewhere; replaced
  with the zero-tests check.
- **V-5's fd probe as a criterion** — the node itself scores 0; demoted to a
  note, with the structural lint test as the criterion.
- **R-1's `service status` id read** — a read of the grant file, presented as a
  read of the daemon; replaced with the restarted process's own
  `compute daemon serving instance=` line.
