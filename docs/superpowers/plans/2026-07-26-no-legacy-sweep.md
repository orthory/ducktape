# No-Legacy Sweep — 2026-07-26

Tree-wide audit against `AGENTS.md` §"No Legacy, No Compat". Read-only; this
file is the only thing the sweep created.

**Base:** `e0d773f68` (dev, "Merge pull request #818"). Read from a clean
`git archive` export, not from a working checkout — the primary checkout is on
`fix/tart-ssh-probe-retry-on-timeout`, 23 commits behind dev, and reading it
would have produced findings about code dev no longer has.

**The rule:** zero live networks, therefore no backward compatibility, no wire
tolerance, no config aliases, no version machinery, no migration. *Dual-path
code is a defect, not prudence.* Numbering is pinned at v1.

**The heuristic:** if a pin test exists to keep two things in sync, the two
things are the defect. A test asserting `a == b` for two independently-derived
values is a confession.

**Counts:** 12 *delete the old half* · 7 *fold two into one* · 24 *dead
decision (doc asserts something untrue)* · 19 *swept and clear*.

---

## Corrections to the brief's premises

Two premises were wrong. Recording them first, because a sweep that reported
them either way would be wrong.

1. **`bin/node/src/resident_announce.rs` was not deleted.** Its deletion is in
   **PR #819, still OPEN**. The file is live and wired (`bin/node/src/main.rs:91`,
   `bin/node/src/replica/park.rs:29,681,948,2078`), as is
   `bin/node/tests/resident_announce_e2e.rs`. `git log --diff-filter=D` on the
   path returns nothing. What *was* removed is the resident dispatch pump
   (`park.rs:670`: "There is no resident dispatch pump any more"). Do not treat
   it as a half-finished deletion; do not delete it outside #819.

2. **PR #819's body documents a dual path that is live on dev right now** — two
   copies of the service-tag rule, the looser one at the hello boundary. Confirmed
   independently below (B1). #819 fixes the symptom host-side and explicitly
   leaves the duplicate predicate in place.

Every other named deletion **was** complete:

| Claimed deletion | Verified |
|---|---|
| service build gate (#820) | Gone. `build_identity` survives as metadata only, guarded by a behavioural test **and** a source lint (`bin/noded/src/services.rs:794`). No admission path reads it. |
| node TEE-embed / `DUCKTAPE_AIRLOCK_SERVE*` | Zero hits outside dated plan files. `bin/node/src/airlock_serve.rs` does not exist. |
| `bin/airlock-broker`, `bin/airlock-cli` | Gone from the tree (`git ls-tree` count 0) and from `[workspace] members`. `seal`/`inspect` folded into `bin/node/src/cred_seal.rs`. |
| `--compute` | One hit, a comment correctly calling it "retired" (`bin/node/src/config/resolve.rs:295`). Not in completions, Makefile, `ops/*.sh`, `skills/`, `docs/records/`. `ops/demo-seed.sh:122-131` correctly uses the replacement flow. |
| flat `sandbox =` key | Gone, not deprecated. `Option<SandboxToml>` + `deny_unknown_fields`; pinned by `resolve.rs:1236-1243` ("*the retired flat `sandbox = "direct"` spelling fails loudly*"). |
| `--attest self-host`, mock upstream | Gone. `bin/airlock-gateway/src/main.rs` builds `AttestMode::Tsm` only; `mock_upstream.rs` deleted. |
| `crates/modules/system/airlock` → `crates/airlock` | Complete; nothing at the old path, and every live doc uses the new one. |

---

## Module = onchain, service = offchain: has the debt grown or shrunk?

**Shrunk.** `crates/modules/system/airlock` — the two-party contract library, no
`MODULE_ID`, no guest wasm — moved to `crates/airlock` in #818 with an explicit
rationale (`Cargo.toml:86-91`). That was the largest known violation; it is
resolved. The six new `crates/services/*` crates are all genuinely offchain,
none carries a `MODULE_ID`, and `git diff --stat crates/modules/` was empty
across the daemon PRs, so the genesis app hash did not move.

Checked every crate under `crates/modules/` for an `sdk::Module` impl. Two
non-modules remain:

- **`crates/modules/system/blobstore`** — host-side counterpart, named as such in
  the layout comment. Pre-existing accepted debt, unchanged.
- **`crates/modules/system/duckdns`** — since `a98c506ed` ("merge duckdns into
  gateway") this is a *naming library* gateway wraps, not a module, and
  `crates/kernel/host` depends on it directly. Same category airlock just left;
  lower urgency (it has a real consumer relationship with gateway), but it is now
  the remaining instance.

---

# Bucket A — delete the old half

### A1. Four live docs assert `app/` was deleted. It wasn't. *(rank 1 — worst)*

**The two paths.** Every operational doc that mentions the desktop app says it
was removed:

- `README.md:173` — "*The product ships as headless surfaces — there is no
  bundled desktop app in this tree.*"
- `skills/qa/SKILL.md:3` — "*The native Iced desktop QA … was retired when app/
  was removed*"; `:11` — "*retired with the removal of app/. There is no desktop
  app to drive in this tree.*"
- `skills/sim-lane/SKILL.md:15` — "*retired with the removal of app/. Only the
  embeddable node below survives.*"
- `ops/README.md:4` — "*there is no desktop app in this tree anymore — the native
  iced shell was removed*"

**Which is current: the code.** `app/` is a live `[workspace] members` entry
(`Cargo.toml:125`), package `ducktape-app`, **9,564 lines of Rust** under
`app/src`, `fn main() -> iced::Result` at `app/src/main.rs:5`, UI in
`app/src/ui/*.ice`, `#[cfg(test)] mod tests` at `:9`. `crates/design` is
described in `Cargo.toml:96` as "*the desktop app's design system*". Last touched
by PRs #803 and #808.

**What happened.** `app/src-tauri` and `app/src-iced` *were* deleted, along with
`app/package.json`, `app/index.html`, `app/bun.lock` — the TypeScript and the old
shell. The app was **rewritten in place**, not removed. Four docs recorded the
deletion and none recorded the replacement.

**What keeps them in sync.** Nothing. No test reads a doc.

**Concrete drift — this is already costing.** `skills/qa/SKILL.md` is a runbook
people follow. Because it states there is no app, its "What to run" section omits
`cargo test -p ducktape-app`, so **every QA pass performed against that skill
silently skips a 9.5k-line workspace member that has tests.** And the next
cleanup pass that trusts four independent docs saying "app/ was removed" will
delete a live member — the docs agree with each other, which is exactly what
makes them convincing.

`ops/README.md` even contradicts its own directory: `ops/demo-seed.sh:5,7-8`
still tells you to "*Open the app and switch to the "demo" workspace*".

**Deletion.** Delete the four claims; give `app/` a row in the README layout
table (it has none) and a line in `skills/qa`. This is the only finding in the
report that is making current work wrong rather than merely risking it.

---

### A2. One question, four hand-written answer ladders: "which node am I talking to" *(rank 2)*

**The paths.** Four sites resolve the node HTTP base, each with a different
precedence, all verified by reading:

| Site | Order |
|---|---|
| `bin/node/src/fs_cli/args.rs:64-81` `resolve_node_addr` | `--node` → `-n/--network` → `DUCKTAPE_NODE` → `None` |
| `bin/node/src/agent_cli.rs:425-449` `own_node_base` | `-n/--network` → `DUCKTAPE_NODE` → lone registered workspace → error. **No `--node` url arm at all.** |
| `bin/node/src/userkey_cli.rs:758-779` `redeem_node` | `--node` → `-n/--network` → error. **`DUCKTAPE_NODE` not honored.** |
| `bin/node/src/cli_args.rs:91-123` `Selector::config_path` | `-n/--network` → `--config` → `./node.toml` → lone workspace → error (resolves a path; env not honored) |

Plus two more that duplicate the *workspace-dir* half under the same struct name:
`services.rs:718-762` `WorkspaceArgs::dir()` (`--config` → `--workspace` → `-n`)
and `gateway_routes.rs:178-198` `WorkspaceArgs::dir()` (`--workspace` → `-n`).
Two structs, one name, two ladders, neither using `cli_args::Selector`.

**Which is current.** None — they are peers. `config::resolve_network`
(`config/mod.rs:786`) is the shared helper and its doc says "*ONE shared resolver
so no family re-walks the registry*" — true, but it owns only the registry-walk
*step*. The **ordering** is four hand-written copies.

**Worse: `--node` means two different things.** Under `fs` and `user cred` it is
an http base URL; under `agent` (`agent_cli.rs:60-63`) it is "*host node to run
on: a display name or a raw 64-hex node key*". Same spelling, incompatible types,
and both land in the same completion buckets.

**What keeps them in sync.** Nothing. No test compares any two ladders.

**Concrete drift — already latent, not hypothetical.** With `DUCKTAPE_NODE` set
and no registry entry: `ducktape fs ls` uses the env; `ducktape agent pty`
consults the env *before* the lone-workspace fallback, so it can pick a different
node than `fs` did; `ducktape user redeem-invite` ignores the env entirely and
errors out. Three answers to one question, in one shell, in one session.

**Stale doc on the same seam:** `bin/node/src/fs_cli/read_cmds.rs:3-4` says
"*every verb resolves the node address the same way (`--node` or
`DUCKTAPE_NODE`)*" — `-n/--network` was added to that ladder and the header
never followed.

**Deletion.** One resolver owning the whole ladder (flag → network → env →
registry), used by all four families; rename `agent`'s `--node` to something
that isn't an http base (`--host-node`), since it is a different type.

---

### A3. `crates/kernel/indexer/src/search.rs` is a dead fork of the live tokenizer *(rank 3)*

**The two paths.** `crates/kernel/indexer/src/search.rs` (140 lines) and
`crates/kernel/index-guest/src/search.rs` (164 lines) are the same algorithm.
`tokens()` is **byte-identical** between them, doc comment included, as are the
constants above it (`MAX_TOKENS_PER_TEXT = 256`, `DEFAULT_POSTING_CAP = 4096`).
The whole-file diff is 73 lines and every difference is mechanical:
`ViewReader` → `impl StateRead`, `Result<T>` → `T`, `reader.scan` →
`reader.scan_page`.

**Which is current.** `index-guest`. It is what `chat` and `pages` call
(`crates/modules/apps/chat/src/index.rs:52,450,468,994,1002`;
`crates/modules/apps/pages/src/index.rs:36,247,254,671,679`). The `indexer` copy
has **zero callers**.

**What keeps them in sync.** Nothing, and worse than nothing:
`crates/kernel/indexer/src/lib.rs` declares only `mod disk;` (74), `mod mem;`
(81), `mod tests;` (1000). **There is no `mod search;`.** Not compiled, not
linted, not tested.

**Measured proof it is dead, not merely unused.** `search.rs:9` reads
`use crate::{Result, ViewReader};`. `ViewReader` exists in exactly **two places
in the entire tree — lines 9 and 65 of this very file**. The type it imports was
deleted; the file could not compile even if you declared it. A full-depth scan of
every `*.rs` under every `src/` for a matching `mod` declaration or `#[path]`
found **this as the tree's only orphaned source file**.

**How it happened.** `6cb6294b3` (2026-07-23, "wasm index guests — the derived
tier's mappers leave the node binary") removed `mod search;` and left the file.

**Concrete drift.** Someone relaxes `filter(|t| t.len() >= 2)` to index
single-char CJK. They grep `fn tokens`, get two hits, and edit the one in the
crate literally named `indexer`. `cargo build -p indexer` passes — the file isn't
in the build — and they ship a change that does nothing. The reverse is equally
likely: fix `index-guest` correctly, then "also update" a file that cannot
compile, burning a review round.

**Damage ceiling:** the index tier is explicitly *not* consensus state
(`indexer/src/lib.rs:10-15`: "never part of any `root()` or the root-hash",
"node-local"), so this cannot fork a chain. It ranks here because it is the one
finding whose only feedback signal is a passing build.

**Deletion.** `rm crates/kernel/indexer/src/search.rs`. Nothing else — `indexer`
already depends on `index_guest` and re-exports from it (`lib.rs:99`).

---

### A4. The broker's `DUCKTAPE_AIRLOCK_*` env arm *(rank 4)*

**The two paths.** `crates/services/broker/src/lib.rs:1572-1585`:

```rust
if let Some(cfg) = explicit { return AnthropicAuth::airlock(cfg).await; }
match AirlockConfig::from_env() {
    Some(cfg) => AnthropicAuth::airlock(cfg?).await,
    None => Ok((AnthropicAuth::from_host()?, ANTHROPIC_MESSAGES_URL.into())),
}
```

**Which is current.** The explicit arm — the consensus-resolved credential record
(`AirlockConfig::self_host(&ResolvedCredential)`), used by
`crates/services/compute/src/pool.rs:1226`, `crates/services/agent/src/lib.rs:527`,
`bin/node/src/compute/cred.rs:149`. The env arm is the PoC-era surface from
`docs/superpowers/specs/2026-07-18-execution-auth-separation-design.md`.

**What keeps them in sync.** Nothing, and the env arm is *unexercised*:

- `AirlockTrust::Attested` — the only thing `from_env` produces — is constructed
  in three places: `from_env` itself (`:1507`) and two broker unit tests
  (`:2920`, `:2964`) that build the struct **directly, bypassing `from_env`**.
- **No file in the repo sets any `DUCKTAPE_AIRLOCK_*` variable** — not a test,
  ops script, Makefile, or harness. Zero `set_var` hits.
- The codex sibling deliberately has no env arm (`resolve_codex_upstream`,
  `:154-163`). Two vendors, two resolution ladders, no stated reason.

**Concrete drift.** The broker now runs inside a long-lived daemon (`ducktape
service run agent|compute`), not a short run. A daemon started from a shell with
`DUCKTAPE_AIRLOCK_GATEWAY` exported — a leftover `.envrc`, a systemd unit —
routes **every claude run's credential traffic through that URL**, silently,
because env beats the host credential. The same daemon's codex runs are
unaffected. That asymmetry is what makes it a bug rather than a feature.

**Not two audiences.** Both arms answer one question — where does this run's
claude credential come from — for one holder, the broker. The trust *model*
differs (attested quote vs pinned on-chain seal_pk), but that is a field on
`AirlockConfig`, not a reason for a second configuration surface.

**Deletion.** Delete `AirlockConfig::from_env`; collapse
`resolve_anthropic_upstream` to the codex shape. To keep the attested/TEE
topology reachable, carry the measurement in the on-chain credential record so
`Attested` comes from the same resolver as `PinnedSealPk`.

---

### A5. The completion drift guard is one-directional — four phantoms and one hole *(rank 6)*

**The guard.** `bin/node/src/cli.rs:1533-1607`
`completion_files_cover_the_verb_table_per_family()` walks the **clap tree** and
asserts each token appears in the completion text. It never walks back. **clap ⊆
completions is checked; completions ⊆ clap is not.** It also matches with
`String::contains` (substring, not token) and inspects only `arg.get_long()`, so
shorts are unchecked in both directions.

Its own doc comment (`:1527-1531`) overclaims: "*renaming a verb or adding a flag
without updating the hand-written completions fails here*" — true; **deleting one
does not.**

**The four phantoms it lets through** (all verified against the clap grammar):

| Phantom | Where | Reality |
|---|---|---|
| `--wireguard-effect` | `ducktape.bash:18`, `ducktape.zsh:15` | No such flag. The repo *deliberately* retired the key, with two dedicated refusal tests (`config/node_toml.rs:463`, `config/resolve.rs:1790`) and an ADR line. Only the flag leaked into the shell files, where nothing looks. |
| `--route-key` | `ducktape.bash:23`, `ducktape.zsh:20` | Never existed on `user`. The only `route_key` in the tree is a gateway storage-key helper. |
| `version` (a top-level family) | `ducktape.bash:12`, `ducktape.zsh:9` | The clap `Family` enum (`main.rs:186-211`) is `Node User Gateway Fs Service Agent Mcp EgressHook`. `ducktape version` is a usage error. Residue: `main.rs:171-172` still refers to `version_line()` (the `version` verb) — a function that exists nowhere. |
| `-m` on `fs commit` | `ducktape.bash:27`, `ducktape.zsh:24` | `fs_cli/mod.rs:141` is `#[arg(long, …)]` with no short. |

**And one real flag it fails to notice is missing.** `user sign-frame --target`
(`userkey_cli.rs:210-212`) is not in `user_flags` — but the guard's
`scope.contains("--target")` is satisfied by the `--target-key` substring, so it
passes while the flag is genuinely uncompleted. This is exactly the hole the
sibling test `the_family_scope_does_not_borrow_a_siblings_verb` (`cli.rs:1611`)
closed for *verbs* and never closed for *flags*.

**The pointer to the guard has itself drifted.** `ducktape.bash:1-3` names
`cli.rs tests::completion_files_cover_the_verb_table` — renamed to
`…_per_family`.

**Deletion.** `clap_complete` is not a dependency today; adding it and generating
both files from `CommandFactory::command()` deletes two hand-written shell files,
the guard, all four phantoms and the substring hole in one move.

---

### A6. `NetworkDescriptor` is the tree's one non-strict parser — on the shareable artifact *(rank 7)*

`bin/node/src/config/mod.rs:51-77` derives `Deserialize` with **no
`deny_unknown_fields`**. Every other config struct in the tree is strict:
`NodeToml` (`node_toml.rs:43`), `SandboxToml` (`:94`), `DevSeedToml` (`:121`),
`Services` (`services.rs:218`), `ServiceGrant` (`:185`), `LocalRoute`/
`LocalRoutes` (`gateway_routes.rs:18,25`), `Hello` (`noded/services.rs:74`).

**The two paths.** A retired key in `node.toml` is a hard parse error naming it —
there is a test for exactly that (`node_toml.rs:479-488`). The same retired key in
`network.toml` is **silently ignored**. And `network.toml` is the one file that
gets pasted between machines.

**Concrete drift.** Rename `coordination` or a `reach` spelling; descriptors in
the wild keep parsing, the old key is dropped on the floor, and the node boots
with the wrong coordination posture instead of refusing. Silent, and the wrong
posture is a privacy setting.

**Deletion.** Add `#[serde(deny_unknown_fields)]`. One line.

---

### A7. `#[serde(default)]` on fields the only writer always writes *(rank 8)*

Four instances where absence should be an error, not a default:

| Field | Why it's always written |
|---|---|
| `NetworkDescriptor.bootstrap` (`config/mod.rs:63`) | No `skip_serializing_if`, unlike its two neighbours `reach` (`:70`) and `coordination` (`:75`) whose defaults are therefore load-bearing and correct. `to_toml` (`:94`) is plain `to_string_pretty`, so `save()` always emits the key. |
| `ServiceGrant.capabilities`, `.scopes` (`services.rs:197-202`) | `save()` (`:333-361`) always writes both, empty arrays included. The struct also carries `deny_unknown_fields` — so it refuses an *extra* key while silently accepting a *missing* one. |
| `Signaling.needs` (`noded/services.rs:374`) | Always serialized by `GET /v1/services`. Its two siblings on the same struct (`:372-373`) carry no default — three list fields, three treatments. `Signaling` also lacks `deny_unknown_fields` while `Hello`, which feeds it, has it. |

**A doc claim the code contradicts, and it is load-bearing.**
`bin/noded/src/services.rs:122-125` justifies deleting the build gate with:

> `[Hello]`, `crate::stream::ClientMsg` and every `agent_service::wire` type carry
> `deny_unknown_fields` **and default nothing**, so a field this build does not
> know is refused and a field it does know **cannot go missing**.

`Hello` defaults three fields (`:85`, `:88`, `:98`). The build gate's deletion was
right for other reasons, but the property its rationale rests on is half-false.

---

### A8. The orphaned shipped-index lane *(rank 10)*

The code confesses this one. Two sites:

- `bin/node/src/boot/env.rs:56-60` — `sync_index: bool`, `#[allow(dead_code)]`,
  "*ORPHANED by the in-process promotion seat … the config key survives until the
  shipped-index lane is swept as one follow-up removal.*"
- `bin/node/src/explorer.rs:239-243` — `stage_shipped_index()`, same note, naming
  a third site ("serve side, IndexStore adoption").

**The defect is the config key, not the function.** A dead private fn is inert.
`sync_index` is a **node.toml key that still parses and steers nothing** — an
operator sets it, gets no error and no behaviour. That is "a config alias kept
just in case"; a comment promising a future sweep is the doctrine's marker for it,
not an exemption. Sweep the lane as the comment already scoped it.

---

### A9. The airlock server's constructor ladder, and three dead decisions *(rank 11)*

`crates/airlock/src/server.rs` exposes **five** public constructors over one
private `assemble`. **Production callers: two** — `build` (the enclave bin,
`bin/airlock-gateway/src/main.rs:83`) and `build_self_host_reloadable` (the lender
service, `crates/services/airlock/src/lib.rs:104`). `build_seeded`,
`build_seeded_gated` and `build_with_quoter` are **called only by tests**: public
API on a shipping crate, which is what `crates/airlock/src/testkit.rs` is for.

**Three doc comments justify the design against the node embed #818 deleted:**

- `:167` — "*The credential-lending **node embed** calls this*" (standalone daemon now).
- `:180` — "*Only the credential-lending **node embed** calls this; every other
  build path leaves the gate off.*" Doubly wrong: the embed is gone, and
  `build_seeded_gated` has **no production caller at all** — the service reaches
  the gate via `build_self_host_reloadable` → `build_self_host`, which never
  passes through it.
- `:201` — "*Used when the credential provider IS the gateway process (the node embed)*".

**A latent dual path worth naming.** `cfg.attest` is authoritative on one entry
point and inert on the other: `build_seeded_gated:186` matches on `AttestMode`;
`build_self_host_reloadable` calls `build_self_host` directly and never reads the
field. Today they agree by luck (`services/airlock/src/lib.rs:95` sets
`AttestMode::SelfHost`). Add a third mode, wire it into the match, and the
reloadable path silently keeps serving self-host with no compile error.

---

### A10. `FORMAT_VERSION` on `services.toml` and `gateway-routes.json` *(rank 12)*

`bin/node/src/services.rs:26,220,237` and `bin/node/src/gateway_routes.rs:15,27,42`
each carry `const FORMAT_VERSION: u8 = 1;`, a required `version: u8` field, and
`if self.version != FORMAT_VERSION { Err("unsupported format version") }`.

This is the machinery `AGENTS.md:13-16` names by name. The check is inert by
construction: the only writer is this code, which always writes `1`, so the
comparison can never be false except for a hand-edited file — and a hand-edited
file is already caught by `deny_unknown_fields` plus the existing structural
validation. Same class as the build-stamp equality check already deleted this
session: a comparison that can never be false, wearing the costume of a gate.

---

### A11. `crates/labs`-shaped code that isn't in `crates/labs` *(rank 13)*

`crates/modules/apps/vaults` implements `sdk::Module`, is a `[workspace] members`
entry (`Cargo.toml:104`) and a `[workspace.dependencies]` entry (`:273`) — and is
referenced by **nothing outside its own test file**. No genesis set, no binary, no
other manifest.

The repo wrote the policy for exactly this case one screen below: `crates/labs`
holds "*quarantined experimental consensus modules … registered by NO genesis set.
**EXCLUDED from the workspace***". `vaults` is that shape without the quarantine,
so it taxes every workspace build and test run. (`README.md:44` correctly notes
`kv` and `vaults` are unregistered — the README is right and the layout is not.)

---

### A12. `bin/coordinator` re-declares one flag list three times, with no test *(rank 14)*

Not a test-pinned pair — an *unpinned* triple, in the one binary that skipped the
clap unification:

1. `USAGE` (`main.rs:16-24`) — the operator-facing list.
2. `validate_args`'s match arm (`:38-39`) — anything else is "unknown coordinator flag".
3. The `arg_value("--…")` call sites (`:175,182,194,198`), each re-scanning
   `std::env::args()` independently (`:31`).

Add a flag to (3) and forget (2) → rejected as unknown. Add to (2) and forget (3)
→ accepted and silently ignored. Add to either and forget (1) → `--help` lies.
Nothing tests any pair. `ops/coordinator/coordinator.env.example` is a fourth
transcription. The `ducktape` binary next door uses clap, which collapses all
three into one declaration.

---

# Bucket B — fold two into one

### B1. `kind_is_well_formed` is implemented twice, byte-identically *(rank 5)*

**The two paths.**

- `bin/noded/src/services.rs:301-307` — the **hello boundary** (what a daemon
  signals). Uses a named `MAX_KIND_LEN = 32` (`:53`).
- `bin/node/src/services.rs:280-286` — the **grant boundary** (`services.toml`
  validation, `service enable`, `service run`). Uses a magic `32`.

Same body, character for character:

```rust
let len_ok = !kind.is_empty() && kind.len() <= 32;
len_ok && kind.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
```

**Which is current.** Neither — peers. The node copy's doc (`:277`) admits the
coupling: "*the same rule the node's hello boundary enforces, so a signaling kind
and a granted kind are always comparable*".

**What keeps them in sync.** Nothing. No test compares them. `bin/node` already
takes a path dependency on `noded` (`Cargo.toml:110`) and already imports the kind
constants from it (`services.rs:33`). The predicate was re-implemented purely
because noded's copy is private (`fn`, not `pub fn`).

**Concrete drift.** Widen one side — say the hello starts admitting `.` so a
daemon can signal `compute.gpu`. It passes the hello, shows in `service list`, the
user runs `service enable compute.gpu`, and the node's validator rejects it. Or
the reverse: widen the grant side and `services.toml` accepts a kind the hello can
never produce, leaving a permanently unmatched grant. Either way the failure
surfaces two layers from the edit.

This matters more after #819, whose correctness argument depends on service kinds
being a strict subset of `capability::validate_tag` — an argument made against
*one* of these two copies.

**The contrast is already in this repo.**
`crates/modules/system/capability/src/interface.rs:63-68`: "*the ONE definition of
a well-formed capability class … every layer that accepts a class validates
through this — no copies to drift.*"

**Fold.** Make `noded::services::kind_is_well_formed` public; delete `bin/node`'s
copy alongside the constants it already re-exports.

---

### B2. `validate_agent_id` vs `duckdns::validate_handle_shape` *(rank 9)*

The purest instance of the heuristic, because the confession is in a manifest.
`crates/modules/apps/agent/Cargo.toml:44-48`:

```
# TEST-ONLY (no layering edge at build time): the agent id rule is a copy of
# duckdns's `.duck` label rule — because the id IS the local part of
# `<agent_id>@agents.duck`. `agent_id_shape_matches_the_duckdns_label_rule`
# pins the copies together so neither can drift alone.
```

`crates/modules/apps/agent/src/lib.rs:99` repeats it: "*deliberately a COPY of
duckdns's `validate_handle` shape rule*".

**What keeps them in sync.** `agent_id_shape_matches_the_duckdns_label_rule`
(`lib.rs:1698-1735`) — a **17-case sample**, not a proof.

**Concrete drift the sample misses.** Both length cases are derived from
**agent's own constant** (`"x".repeat(MAX_AGENT_ID_LEN)` and `+1`). If duckdns's
maximum label length grows to 64, every case still agrees, the test stays green,
and the two rules are genuinely different for any id between the two limits. The
charset cases catch a *narrowing* of duckdns but not a *widening* into a character
the sample doesn't happen to contain.

**The stated reason for the copy is stale.** "No layering edge at build time" was
live when duckdns was a consensus module. Since `a98c506ed`, duckdns **is** a
shared naming library, and `crates/kernel/host` and `crates/modules/system/gateway`
already take ordinary build-time dependencies on it.

**Fold.** Promote `duckdns` to `[dependencies]` in `agent`; make
`validate_agent_id` call `duckdns::validate_handle_shape`; delete the pin test.
Keep the second half of that test (the `RESERVED_ROOT_LABELS` block) — it asserts
a *real* policy difference and is not a pin.

---

### B3. `state_tokens_match_the_rendered_labels` *(rank 15)*

`bin/node/src/services.rs:2102-2117` asserts each `ServiceState`'s serde token
equals its `label()`. Two derivations of one string: the `#[serde(rename = ...)]`
attributes (`:377,380,384`) and the `match` in `label()` (`:505-511`). The struct's
own doc is the confession (`:371-373`): "*The serde spellings are written out
rather than derived from a rename rule: they must equal `ServiceState::label`
exactly … `state_tokens_match_the_rendered_labels` pins that.*"

**Drift.** Add a fourth state, forget one side: `--json` says one thing and the
table another. The test catches it — which is the point. **Fold:** a hand-written
`Serialize` that calls `label()`, then delete the renames and the test.

---

### B4. The pre-refactor formula pins *(rank 16)*

Two tests keep a deleted implementation alive inside the test body:

- `crates/kernel/sdk-testkit/src/lib.rs:307`
  `memstore_root_matches_the_pre_refactor_inline_formula` — hand-rolls the old
  preimage, hashes it, asserts it equals `MemStore::root()`. **No golden
  constant**: the assertion is entirely "the function equals my copy of the function".
- `crates/kernel/sdk/src/hash.rs:52` `encode_pairs_golden_matches_inline_formula`
  — *does* have a hand-built golden byte vector (correct, valuable), then
  additionally re-derives `inline` with a loop identical to `encode_pairs`.

**Drift.** These pin the map-hash preimage, which every map-backed module root
depends on — worth pinning. But a re-derivation is the wrong instrument: when
`encode_pairs` changes, the test fails and the obvious repair is to update the
copy, silently re-blessing a moved root. A frozen golden constant cannot be
"fixed" that way without visibly changing a magic number.

**Fold.** Delete the `inline` half of `hash.rs`'s test; replace sdk-testkit's
re-derivation with a golden root constant.

---

### B5. Constants duplicated across crates *(rank 17)*

| Value | Sites |
|---|---|
| `"127.0.0.1:8844"` | `config/node_toml.rs:27` (`DEFAULT_HTTP_LISTEN`, written into every generated `node.toml`) **and** `bin/noded/src/main.rs:61` (hand-rolled default). `README.md:181`, `docs/dogfood.md:44`, `skills/qa/SKILL.md:32` all treat `:8844` as one number. |
| TCP `443` | `config/node_toml.rs:326` (`derive_coordinator_relay`), `bin/coordinator/src/main.rs:184` (`--relay-listen` default), `:20` (the USAGE string). Three copies. |
| UDP `3478` | `config/mod.rs:364` (`DEFAULT_PRIMARY_COORDINATOR`), `bin/coordinator/src/main.rs:177,19`. |
| `"docker.io/library/node:22-slim"`, `"ghcr.io/…/macos-sonoma-base:latest"` | `config/resolve.rs:283,286` **and** `crates/services/provider/src/lib.rs:3923,3941`. Provider copies are inside `#[ignore]`d hardware tests — low blast radius, still two spellings of one default. |

Nothing asserts any pair equal.

---

### B6. Two `MAX_REQUESTS = 4096` that look like twins and are not *(rank 18)*

`crates/services/airlock/src/lib.rs:36` (how many upstream requests one scoped
session token the **lender's** gateway mints may make) and
`crates/services/broker/src/lib.rs:45` (the **borrower's** run-scoped ceiling).
Different holders, different layers, genuinely two jobs — **not** a dual path. But
neither comment mentions the other, and the identical value reads as coupled.
Raise the broker's to 8192 for longer sessions and the lender's 4096 silently
becomes binding, surfacing as a 429 from the wrong layer. **Fix is a
cross-reference comment, not a merge.**

---

### B7. `PortPolicy::production()` — a constructor production never calls *(rank 19)*

`crates/networking/wireguard/src/lib.rs:170-178`. Every caller is a test; the live
reachability plane uses `reachability::open_port_policy()`
(`bin/node/src/reachability_plane.rs:490` → `binding.rs:70-79`). So its hardcoded
`51820`/`443` are **not** a second source of truth for `DEFAULT_WIREGUARD_LISTEN`
— but the name asserts otherwise, which is how it becomes one. Rename or delete.

---

# Bucket C — dead decisions (a doc asserts something untrue)

All verified against the tree. `docs/superpowers/plans|specs/` are dated
historical records and were excluded; everything below is a **live** surface.

### C1. `README.md` — the layout table names five paths that don't exist

| Line | Claim | Reality |
|---|---|---|
| `:38` | `bin/` contains `fs` (duckfs CLI) and `mcp` (MCP tool server) | No `bin/fs`, no `bin/mcp`. Both are `ducktape` subcommands (`main.rs:198,205`). `Cargo.toml:50-51` gets this right, so README contradicts Cargo.toml. |
| `:32`, and `Cargo.toml:24` | a `jobs` module | No such crate. The job board is `crates/modules/apps/tasks/src/job_board.rs`. |
| `:31` | system modules include `clients` and `dispatch-oracle` | Neither exists; `duckdns` and `gateway` are missing from the list. |
| `:30` | `crates/networking/` holds the consensus modules `duckdns` and `gateway` | It holds none — `Cargo.toml:16-18` says so explicitly and is correct. The line is also self-contradictory: it lists `duckdns` *and* calls `gateway` the module that absorbed it. |
| `:27-39` | — | No row for `crates/airlock/` (the crate #818 created), `crates/design/`, `crates/rpc-client/`, `crates/testing/`, or `app/`. |
| `:176-183` | "*Release build with `make node`; run a throwaway dev daemon with `cargo run -p noded`*" under the heading **`node-bin`** | `make node` builds `-p node-bin` (binary `ducktape`); `cargo run -p noded` runs a *different* binary. |

### C2. `README.md:53-82` — eleven merged PRs advertised as unmerged

`:54` "*their PRs are open and unmerged*", `:60-61` "*#724–#728 are open and
unmerged*", and ten table rows tagged "(this campaign, PR #71x/#72x — unmerged)".
Every named symbol is in the tree today (`MemBlobs`, `MemRefs`/`DiskRefs`,
`MemDisk`, `SimMesh`, `StepOrderer`, `ModuleTopology`, `worker::drive`,
`project_block`, `crates/kernel/sdk-testkit/`). Merged PRs on dev are now
#801–#826.

### C3. `skills/module-dev/SKILL.md` sends you to edit a construct that no longer exists

`:76` "*`constants.rs`: `MODULE_IDS` (bump the `[..; N]` literal)*", `:85-86` and
`:105` (a whole row about "the auto-merge count trap"). But
`bin/node/src/constants.rs:148` is now
`pub(crate) const MODULE_IDS: &[&str] = host::topology::PRODUCTION;` — a slice, no
count array, no trap. The id universe lives in `crates/kernel/host/src/topology.rs:123`,
whose own `//!:5-11` says the four per-bin lists were unified *precisely to kill
this drift class*. `:77-79` ("`bin/noded` … id list", "`bin/simnode` same shape",
"`bin/demo` same shape") is likewise stale: `grep MODULE_IDS` in all three returns
**zero hits**. The whole "§3 Registration — four bins compose modules" section
needs rewriting against `ModuleTopology`.

### C4. `skills/qa/SKILL.md:26` — dangling cross-reference

Points at `sim-lane` for "*the `iced_test::Simulator` traps*";
`skills/sim-lane/SKILL.md:13-15` says that half was retired and does not document
it. `iced_test` appears nowhere in the tree.

### C5. `skills/sim-lane/SKILL.md:13-15` — a path that never existed in this shape

Names `app/src-iced/src/shell/sim/`. The app's source root is `app/src/`.

### C6. `crates/airlock/README.md` — the crate it names was renamed, and its Deferred list shipped

`:229,240` say `capability-host`; the crate is `broker-host` at
`crates/services/broker` (`capability-host` appears nowhere outside dated docs).
The same file's `:250` correctly says `cargo test -p broker-host airlock` — both
names in one README. Separately, `:254-263` "## Deferred" defers body-level AEAD
and SSE-over-overlay streaming while `:130` and `:137` of the same file say both
shipped (2026-07-20 / 2026-07-23).

### C7. `docs/records/specs/capability-spec.md` — the crate it documents has moved

No date banner; reads as the current v1 reference. `:8`, `:16`, `:56`, `:569` all
point at `crates/modules/system/capability-host/…`; the specs and the host are now
`crates/services/provider/` (`provider-host`). (`:15`, pointing at
`crates/modules/system/capability`, is correct.)

### C8. `docs/records/architecture/wasm-module-authoring.md:9` — the "first REAL production tenant" path doesn't exist

Names `crates/guests/directory-wasm`. `directory` carries its own port at
`crates/examples/directory/src/guest.rs`, built via `Makefile:102`. This
contradicts `README.md:35` and `Cargo.toml:33-39` ("no per-module crate lives
here"). Everything else in the file checks out.

### C9. `docs/records/architecture/2026-07-16-system-architecture.md` — describes the retired Tauri/React desktop

Self-limits at `:3` ("*as it exists on dev at 256cd887*"), so ranked below C7/C8 —
but it lives in the tier the doctrine calls current and its §12 "Known drift" list
makes it read as maintained. `:52-53,83,99,771-801` reference `app/src-tauri` and a
React webview; `:73-74,756,801` the nonexistent `bin/mcp`/`bin/fs`; `:340,359`
`oracle_pool.rs` in two crates (neither exists); `:142,170-171,679-687,732`
`dispatch-oracle` and `capability-host`. Its own §12 item 1 — a note that the
README layout table is stale — is itself stale, naming five names the README no
longer contains.

### C10. Obsolete invitation protocol deep dive — removed

The stale analysis was removed with the language-specific docs cleanup. Its
maintained replacement is `docs/adr/2026-07-17-join-protocol.mdx`.

### C11. Nine crate doc comments naming dead paths

Lowest severity, but this is the surface the brief calls most load-bearing.

| Location | Says | Actually |
|---|---|---|
| `bin/noded/src/log.rs:8` | "`bin/fs` and `bin/mcp` are CLIs" | `ducktape fs` / `ducktape mcp` |
| `bin/node/tests/mcp_support/mod.rs:4` | "the same shape as `bin/fs`'s harness" | same |
| `crates/services/provider/src/lib.rs:108` | "consumed by `bin/mcp`" | `bin/node/src/mcp/` |
| `crates/services/compute/src/soul.rs:58` | "`bin/mcp/src/tools/read.rs`" | `bin/node/src/mcp/` |
| `bin/node/src/util.rs:14` | "`app/src-iced/src/backend/workspace_service.rs`" | `app/src/backend.rs` |
| `crates/modules/apps/chat/src/call_wire.rs:4` | "the app's TypeScript leg (`app/src/domain/call-frames.ts`)" | no TS in `app/` |
| `crates/modules/system/duckdns/src/wire.rs:20` | "the app's `RESERVED_ROOT_LABELS` … pinned to this literal by `duckdns-client.test.ts`" | both gone — so the "every other copy mirrors it" invariant is **unguarded**, not merely mis-cited |
| `bin/coordinator/src/lib.rs:97` | "`bin/node/src/config.rs`" | now a directory |
| `crates/modules/apps/chat/src/video/mod.rs:3` | "`docs/adr/2026-07-06-video-call-module.md`" | `.mdx` |

### C12. `Cargo.toml`'s layout comment has fallen behind its own members list

`:25-30` lists four service crates (`compute`, `sandbox`, `broker`, `provider`) and
omits `crates/services/agent` and `crates/services/airlock` — the crate #818
created; both are members at `:108-113`. `:50-54`'s `bin/` list omits
`airlock-gateway` (member at `:122`). The `layout:` block omits `crates/airlock/`,
`crates/rpc-client/`, `crates/design/` and `app/`.

### C13. Two soft ones

`AGENTS.md:23` presents `skills/` as `(qa, sim-lane)`; it also holds `module-dev`,
`project-librarian`, `project-orchestrator`, and `README.md:169` calls
`skills/module-dev/SKILL.md` "the full wiring runbook". `AGENTS.md:125` refers to
"the `rust` skill", which is not in `skills/` (may be user-global per `:26-27`;
unverifiable from the tree).

---

# Adjacent — flagged, not a legacy finding

**Service grant `scopes` are declared, displayed, and enforced nowhere.**
`scopes_for()` (`bin/node/src/services.rs:1184-1203`) returns per-daemon scope
tokens; they ride the hello, land in the `ServiceGrant`, and are painted on the
consent screen (`:665`) and in `service status` (`:622`). No code path checks a
grant's scopes before serving a request on the service link.

The doc at `:1174` says "*inventing one the code does not honor would make the
consent screen a lie*" — the tokens are honest *descriptions*, but nothing
*constrains* the daemon to them.
`docs/superpowers/plans/2026-07-26-wave3-scope-enforcement.md` exists, so this is
known unbuilt work, not rot. Included because "a gate that gates nothing" is the
same shape as the version checks above, and a consent screen is the wrong place to
carry an unenforced promise.

---

# Swept and clear — do not re-audit these

Recorded so the next sweep does not rediscover that these are fine.

**Correctly two things doing two jobs:**

1. **Service-link token vs admin token.** Two secrets deliberately — the service
   link goes to the agent daemon, which admin must exclude. They already share
   **one writer**: `noded::services::mint_secret_file`/`read_secret_file`
   (rationale at `bin/noded/src/services.rs:192-193`). The resolved-correctly case.
2. **`AttestMode::Tsm` vs `SelfHost`.** Two deployment shapes, different trust
   anchors (hardware quote vs on-chain seal_pk), both live. The *constructor
   ladder* around them is A9; the enum is fine.
3. **`compute_backend` vs `service.sandbox`.** Two values from one `[sandbox]`
   table differing by the compute grant, because the pty plane and the airlock
   gateway must work on a node that granted no compute. Defended in place
   (`config/resolve.rs:134-137`).
4. **First-contact UDP race vs `RelayFallback`.** A transport fallback for a
   network condition, not an older protocol.
5. **`AnthropicAuth::from_host`.** A genuinely different holder from a lent
   credential. Only the *env* arm beside it is the finding (A4).
6. **`DUCKTAPE_HOME`** has no toml twin — it *is* the registry root, and there is
   no config file above the registry.

**Mirrors correctly pinned against the real thing:**

7. **`runs` ↔ `forge` wire mirrors.** Five "mirror matches" tests, but each
   round-trips through **forge's own codec** (`forge::decode_query`,
   `forge::decode_msg`), not a hand-copied rule. The constraint is real and
   current: forge drags vendored libgit2, which must stay out of the wasm guest
   graph, so it is `[dev-dependencies]`. Clean end-state is a types-only
   `forge-wire` crate — a structural refactor, not a no-legacy defect.
8. **`runs` ↔ `dispatch` saga-id mirror.**
   `saga_id_mirror_matches_the_dispatch_modules_derivation` drives the **real
   `DispatchModule`** and reads the id off an emitted trigger.

**Deletions done thoroughly:**

9. **The build gate.** A behavioural test proving no stamp value is compared, plus
   a file-granularity source lint (`no_admission_path_reads_this_node_s_build_stamp`)
   with a documented reason for file-granularity over the brace-counting parse it
   replaced. The surviving `Skew` type warns and gates nothing. *(Its rationale
   comment overclaims — see A7.)*
10. **The `user.key` legacy plaintext decoder.** Prefix is
    `ducktape-user-key-v1:` — renumbered to v1 per doctrine, not bumped to v2 —
    and `bare_hex_is_rejected` (`bin/node/src/userkey.rs:462`) pins the refusal of
    the 64-hex format. *(The argon2 params stored in the file, justified at `:23`
    as "raised later without breaking old files", is **not** a compat shim: a
    user's own on-disk key must stay decryptable by its own params or the identity
    is unrecoverable. A re-audit will want to flag this; don't.)*
11. **The `podman run` CLI argv path.** Gone; podman is socket-only. The surviving
    `run_argv` (`crates/services/sandbox/src/sandbox.rs:274`) is **Tart's** CLI,
    which has no socket API.
12. **`wireguard_effect`.** The retired node.toml key is an unknown-field error —
    the doctrine's correct behaviour — with two dedicated refusal tests. Only the
    *flag* leaked into completions (A5).
13. **`the_service_view_agrees_with_the_node_view`, deleted not weakened.**
    `config/resolve.rs:1543-1551` records why: "*`Resolved` now CONTAINS the
    `ServiceConfig` it used to duplicate, so the comparison it made is `x == x` …
    Drift is not tested for because it can no longer be written down.*" **This is
    the template for B1–B4.**

**Verified clean by measurement:**

14. **`#[serde(alias)]`: zero hits tree-wide.** No config key has a second spelling.
    **Zero clap aliases** — no `#[arg(alias)]`, no `visible_alias`, no
    `#[arg(env)]`. Every `-n`/`--network` pair is one short for one long.
15. **Orphaned source files: exactly one in the tree** (A3). Full-depth scan of
    every `*.rs` under every `src/`. (`bin/coordinator/src/bin/coordinator-load.rs`
    is a cargo auto-discovered binary target and needs no `mod`.)
16. **`#[allow(dead_code)]` in shipping code: five sites.** Two are A8; the rest
    are local and benign.
17. **The invite/join flow is unversioned on purpose.**
    `bin/node/src/config/invite.rs:15,295` — "*UNVERSIONED on purpose … ONE format
    … a format change re-mints*". `MeshVersion` is a 32-byte epoch identifier, not
    a protocol version. No `v2` hint anywhere in the flow.
18. **The agent daemon wire is strict and exemplary.**
    `crates/services/agent/src/wire.rs`: `deny_unknown_fields` both directions and
    **no defaults at all**, including
    `#[serde(deserialize_with = "Option::deserialize")]` (`:95`) to close the hole
    where serde lets an `Option` field go missing without a `default`. Its test
    (`:265-290`) exercises both an unknown field and an omitted one.
19. **`NodeToml` is the best-behaved config struct in the tree** — every key
    required, `deny_unknown_fields`, sentinels as explicit values never missing
    keys, one three-layer merge with idempotence and hand-edit survival tested.
    **`simnode` serves noded's router verbatim** (`bin/simnode/src/lib.rs:407-420`)
    — one `admin_guard`, one exposure ladder. **`Makefile` is fully accurate** —
    all 16 `BUILDER_MODULES`, all 6 `INDEX_MODULES`, every `-p` target and every
    `ops/*.sh` it invokes resolve. **No migration machinery**: every `fs::rename`
    is an atomic tmp→final write; nothing reads an older on-disk shape.

---

## Method and confidence

Read from a clean export of `e0d773f68`. The first pass was thrown away and
redone: a stale artifact in the scratchpad produced two phantom findings
(`crates/modules/system/airlock` and `bin/airlock-*` appearing to survive their
deletion), both disproved with `git ls-tree` against the commit before anything
was written down. Deletion completeness was checked against
`git log --diff-filter=D` and each PR's own diff, never against its description.

Claims were proved statically where a static proof is decisive. **A3 rests on
Rust module semantics plus a deleted type** — `ViewReader` exists only inside the
dead file — which is stronger evidence than a build would be. Where the claim was
about *usage* rather than compilation (A4's "nothing sets these env vars", A11's
"nothing depends on vaults"), it was established by exhaustive grep over every
file type that could set or depend, not over `*.rs` alone.

The config/CLI and doc-drift sweeps were dispatched to parallel agents. **Five of
their top claims were re-verified by hand before inclusion** — `app/` being a live
9,564-line workspace member, `--wireguard-effect` and the `version` family being
absent from the clap grammar, `NetworkDescriptor` lacking `deny_unknown_fields`,
and the three divergent node-address ladders — and all five held. Findings from
those sweeps that were *not* independently re-checked are the lower-ranked doc
entries in Bucket C; they are cited with exact line numbers so each is a
one-command verification.
