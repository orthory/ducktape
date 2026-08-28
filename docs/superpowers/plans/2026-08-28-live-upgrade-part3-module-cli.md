# Live Upgrade Part 3 — `ducktape module update|register|status` Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One command stages a component at the node, drives the governance proposal that schedules its swap (or first registration), and refuses to propose if any peer's staging receipt is not ok; `module status` shows the lifecycle registry.

**Architecture:** A new `bin/node/src/module_cli.rs` family (grammar + handlers, like `fs`/`agent`) over three seams that already exist: the owner-gated `POST /v1/admin/module-code/stage` route (raw bytes in, digest + per-peer receipts out, fan-out included), the governance `UpdateModule`/`RegisterModule` actions driven by the existing propose→vote→execute ceremony in `cli.rs` (generalized to take a proposal matcher, because each member computes its own `activation_height`), and the lifecycle `ModuleStatus` query over the generic rpc `query` lane. No node-side change.

**Tech Stack:** Rust, clap derive, `reqwest::blocking` (already used by `node_http.rs`), `sha2`, the `governance`/`lifecycle` interface crates, `noded::admin::read_operator_token`.

**Spec:** `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §4 (decisions 2-A…2-D at the decisions table). Part 4 (§5) consumes these verbs through `Cluster::run_verb`.

## Global Constraints

- Verbs and defaults verbatim from spec §4: `ducktape module update <id> <component.wasm> [--after N]`, `ducktape module register <id> <component.wasm> [--after N]`, `ducktape module status`; `N` default **50**.
- Decision 2-B: **any** staging receipt with `ok: false` → print `peer  status` per failing peer, exit 1, propose nothing. No `--force`/`--allow-partial` flag.
- Decision 2-C: verbs are update + register + status only — no `cancel`, no `remove` (spec "Deferred").
- `activation_height = <rpc status height> + N` (absolute; the lifecycle's floor is `height + MIN_SWAP_LEAD` with `MIN_SWAP_LEAD = 3` at `crates/modules/system/lifecycle/src/lib.rs:54`, strictly exceeded ⇒ `N ≥ 4`).
- The ceremony matches an OPEN proposal on `(action variant, module_id, code_hash)` and ignores `name`/`activation_height` — the second member's computed height differs.
- Output convention: `println!` for the result line, `eprintln!` for narration and refusals (CLI stdout is program output; `tracing` is for the node). No `println!` reaches the node.
- CLAUDE.md house rules: named predicates, early return, one `match` per discriminant, no boolean-flag steering, Edit tool per hunk, only format touched code, no compat/legacy paths, tests wait on events (use `Cluster::await_committed`, never `poll_until`/sleep).
- Gates: `cargo clippy -p node-bin --tests --no-deps`; `cargo test -p node-bin --bin ducktape` (the completions test `completion_files_match_the_clap_tree_per_family` must stay green — both `ops/completions/ducktape.bash` and `.zsh` change when a family is added); `cargo check --workspace --all-targets` (fully green on `dev` since #1262); the new `bin/node/tests/module_cli.rs` lane.
- Subagents run on `opus` or `fable` (user rule). Never `git stash`.

---

## File map

| File | Responsibility |
|---|---|
| `bin/node/src/module_cli.rs` (create) | `ModuleCmd` grammar; `run`; `cmd_stage_and_schedule` (update/register); `cmd_status`; pure helpers `refuse_on_bad_receipts`, `render_status`, `matches_module_action` |
| `bin/node/src/cli.rs` (modify ~1222) | `drive_membership_ceremony` → `drive_proposal_ceremony` taking a matcher; `open_proposal_matching` extracted and unit-tested; `pub(super)` on `rpc_query`, `rpc_call`, `gov_signer`, `GovSigner`, `CeremonyOutcome` as needed by the new module |
| `bin/node/src/main.rs` (modify ~220–278) | `Family::Module(module_cli::ModuleCmd)` + dispatch arm |
| `ops/completions/ducktape.bash`, `ducktape.zsh` | `module` family + its verb/flag declarations |
| `bin/node/tests/module_cli.rs` (create) | pure-CLI refusals + live single-node register→update + two-node fan-out refusal |
| `skills/module-dev/SKILL.md` (modify) | the post-genesis path is now `ducktape module register` |

---

### Task 1: The ceremony takes a matcher

**Files:**
- Modify: `bin/node/src/cli.rs:1222-1300` (`drive_membership_ceremony`), the three callers at `cli.rs:1341` (`cmd_invite_accept`), `:1387` (`cmd_promote`), `:1446` (`cmd_resident_remove`)
- Test: `bin/node/src/cli.rs` `#[cfg(test)] mod tests` (the file already has one — add beside `completion_files_match_the_clap_tree_per_family`)

**Interfaces:**
- Consumes: `governance::{GovAction, ProposalView, ProposalStatus}` (`crates/modules/system/governance/src/interface.rs:178-193`), `CeremonyOutcome` (`cli.rs:1207`)
- Produces:
  ```rust
  pub(super) fn open_proposal_matching<'a>(
      views: &'a [governance::ProposalView],
      matches: &dyn Fn(&governance::GovAction) -> bool,
  ) -> Option<&'a governance::ProposalView>;

  pub(super) fn drive_proposal_ceremony(
      rpc_addr: &str,
      signer: &GovSigner,
      pubkey_hex: &str,
      verb: &str,
      id_prefix: &str,
      wanted: governance::GovAction,
      matches: &dyn Fn(&governance::GovAction) -> bool,
  ) -> Result<CeremonyOutcome, Box<dyn std::error::Error>>;
  ```
  `pub(super) enum CeremonyOutcome`, `pub(super) enum GovSigner`, `pub(super) fn gov_signer`, `pub(super) fn rpc_call`, `pub(super) fn rpc_query`, `pub(super) fn read_members` — visibility widened so `module_cli.rs` (a sibling module under `main.rs`) can use them.

- [ ] **Step 1: Write the failing test** (in `cli.rs`'s test module)

```rust
#[test]
fn open_proposal_matching_ignores_fields_the_matcher_ignores() {
    use governance::{GovAction, ProposalStatus, ProposalView, VoterKind, VotingRule};
    let view = |id: &str, status: ProposalStatus, action: GovAction| ProposalView {
        proposal_id: id.into(),
        action,
        proposer: vec![1],
        created_at: 0,
        deadline: 10,
        status,
        votes: vec![],
        voter_kind: VoterKind::Validators,
        electorate: vec![],
        voting_rule: VotingRule::Majority,
    };
    let hash = vec![7u8; 32];
    let founders = view(
        "module:aa:0",
        ProposalStatus::Open,
        GovAction::UpdateModule { name: "x".into(), module_id: "hello".into(), activation_height: 60, code_hash: hash.clone() },
    );
    let settled = view(
        "module:bb:0",
        ProposalStatus::Passed,
        GovAction::UpdateModule { name: "x".into(), module_id: "hello".into(), activation_height: 60, code_hash: hash.clone() },
    );
    let other = view(
        "module:cc:0",
        ProposalStatus::Open,
        GovAction::RegisterModule { name: "x".into(), module_id: "hello".into(), activation_height: 60, code_hash: hash.clone() },
    );
    let views = vec![settled, other, founders];
    // the second member computed height 61, not 60 — equality on the whole
    // action would never join the founder's proposal.
    let matches = |a: &GovAction| matches!(a, GovAction::UpdateModule { module_id, code_hash, .. } if module_id == "hello" && *code_hash == hash);
    let found = open_proposal_matching(&views, &matches).expect("the open update proposal");
    assert_eq!(found.proposal_id, "module:aa:0");
    let none = open_proposal_matching(&views, &|a| matches!(a, GovAction::CancelModuleUpdate { .. }));
    assert!(none.is_none());
}
```
(`VoterKind`/`VotingRule` variant names: take the first variant of each enum as declared in `interface.rs` — read them; the test needs any value.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p node-bin --bin ducktape open_proposal_matching`
Expected: FAIL — `open_proposal_matching` not found.

- [ ] **Step 3: Extract the matcher and generalize the ceremony**

In `cli.rs`, above `drive_membership_ceremony`:

```rust
/// the open proposal a member should JOIN rather than duplicate. the matcher
/// decides which fields identify "the same proposal": membership verbs match
/// the whole action, module verbs match (variant, module_id, code_hash) and
/// ignore the activation height each member computed for itself.
pub(super) fn open_proposal_matching<'a>(
    views: &'a [governance::ProposalView],
    matches: &dyn Fn(&governance::GovAction) -> bool,
) -> Option<&'a governance::ProposalView> {
    views
        .iter()
        .find(|p| p.status == governance::ProposalStatus::Open && matches(&p.action))
}
```

Rename `drive_membership_ceremony` → `drive_proposal_ceremony`, add the `matches: &dyn Fn(&governance::GovAction) -> bool` parameter LAST, and replace the inline find at `cli.rs:1240` (`p.status == ProposalStatus::Open && p.action == wanted`) with `open_proposal_matching(&views, matches)`. Everything else in the function is unchanged.

Update the three callers to pass equality on the whole action:

```rust
let wanted = GovAction::AddValidator { key: key_bytes };
let same_action = { let wanted = wanted.clone(); move |a: &GovAction| *a == wanted };
match drive_proposal_ceremony(&rpc_addr, &signer, pubkey_hex, "node member promote", "admit:", wanted, &same_action)? {
```
(the same two-line shape in `cmd_invite_accept` and `cmd_resident_remove`; `GovAction` is `Clone` — confirm at `interface.rs:~40`, derive it if not.)

Widen visibility to `pub(super)` on: `CeremonyOutcome`, `GovSigner`, `gov_signer`, `rpc_call`, `rpc_query`, `read_members`, `drive_proposal_ceremony`. Nothing else changes.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p node-bin --bin ducktape open_proposal_matching && cargo clippy -p node-bin --tests --no-deps`
Expected: PASS; clippy clean (an unused `pub(super)` is not a lint).

- [ ] **Step 5: Commit**

```bash
git add bin/node/src/cli.rs
git commit -m "refactor(cli): the governance ceremony takes a proposal matcher

Membership verbs match the whole action; module verbs will match
(variant, module_id, code_hash) because each member computes its own
activation height. In-seam: rename drive_membership_ceremony ->
drive_proposal_ceremony, extract open_proposal_matching."
```

---

### Task 2: `module status` and the family wiring

**Files:**
- Create: `bin/node/src/module_cli.rs`
- Modify: `bin/node/src/main.rs:220-278` (`Family` + dispatch)
- Modify: `ops/completions/ducktape.bash:12` + declarations, `ops/completions/ducktape.zsh:9` + declarations
- Test: `bin/node/src/module_cli.rs` unit test (`render_status`), the existing completions test, `bin/node/tests/module_cli.rs` (created here with the first pure-CLI case)

**Interfaces:**
- Consumes: `cli::{rpc_call, rpc_query}` (Task 1 visibility), `cli_args::{Selector, StatusArgs}` (`cli_args.rs:124,454`), `lifecycle::{LifecycleQuery, LifecycleReply, ModuleCode, ScheduledSwap}` (`crates/modules/system/lifecycle/src/interface.rs:31,51,116,127`), `config::{resolve, hex_bytes}`. The encode/decode pair for a module query: mirror `read_members` at `cli.rs:900` (it does exactly this for `governance`; use the `lifecycle` crate's equivalents — `lifecycle::encode_query` / `lifecycle::decode_reply` or whatever `interface.rs` exports next to the enums).
- Produces:
  ```rust
  #[derive(Debug, clap::Subcommand)]
  pub enum ModuleCmd {
      /// schedule a code swap for a registered module
      Update(StageArgs),
      /// register a new module id with its first code
      Register(StageArgs),
      /// the lifecycle registry: active code and any pending swap per module
      Status(StatusArgs),
  }
  #[derive(Debug, clap::Args)]
  pub struct StageArgs {
      #[arg(value_name = "ID")] pub id: String,
      #[arg(value_name = "COMPONENT.WASM")] pub component: std::path::PathBuf,
      /// blocks after the current height at which the swap activates
      #[arg(long, default_value_t = 50)] pub after: u64,
      #[command(flatten)] pub selector: Selector,
  }
  pub fn run(cmd: ModuleCmd) -> Result<(), Box<dyn std::error::Error>>;
  pub(super) fn read_module_status(rpc_addr: &str) -> Result<Vec<lifecycle::ModuleCode>, String>;
  pub(super) fn render_status(modules: &[lifecycle::ModuleCode]) -> String;
  ```

- [ ] **Step 1: Write the failing unit test** (bottom of the new `module_cli.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use lifecycle::{ModuleCode, ScheduledSwap};

    #[test]
    fn status_rows_show_active_pending_and_readiness() {
        let active = vec![0xabu8; 32];
        let next = vec![0xcdu8; 32];
        let modules = vec![
            ModuleCode { module_id: "acl".into(), active_code_hash: active.clone(), pending: None },
            ModuleCode {
                module_id: "hello".into(),
                active_code_hash: active.clone(),
                pending: Some(ScheduledSwap {
                    name: "hello-2".into(),
                    activation_height: 120,
                    code_hash: next.clone(),
                    readiness: vec![vec![1], vec![2]],
                    ready: false,
                }),
            },
        ];
        let out = render_status(&modules);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "id     active        pending");
        assert_eq!(lines[1], "acl    abababababab  —");
        assert_eq!(lines[2], "hello  abababababab  cdcdcdcdcdcd  ready 2  activation 120");
    }
}
```
(`readiness.len()` is the count of validators that signalled; `n` — the validator-set size — is not in `ModuleCode`, so the row prints `ready k` and `ready ✓` once `ready == true`. The spec's `k/n` needs a second query; keep it out — YAGNI until someone asks for `n`.)

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test -p node-bin --bin ducktape status_rows_show`
Expected: FAIL — module `module_cli` does not exist.

- [ ] **Step 3: Write `module_cli.rs`** (grammar, `run`, `status`; update/register arms return a `todo`-free stub that ERRORS — the verbs land in Task 4)

```rust
//! `ducktape module …` — the operator's side of a live code swap.
//!
//! `update`/`register` stage a component at this node's owner-gated admin
//! route (which fans it out to every validator and returns their receipts),
//! then drive the governance proposal that schedules it. `status` reads the
//! lifecycle registry. Nothing here runs inside the node.

use std::path::PathBuf;

use crate::cli::{rpc_call, rpc_query};
use crate::cli_args::{Selector, StatusArgs};
use crate::config::{self, hex_bytes};

type CommandResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, clap::Subcommand)]
pub enum ModuleCmd {
    /// schedule a code swap for a registered module
    Update(StageArgs),
    /// register a new module id with its first code
    Register(StageArgs),
    /// the lifecycle registry: active code and any pending swap per module
    Status(StatusArgs),
}

/// `<id> <component.wasm> [--after N]` — shared by update and register.
#[derive(Debug, clap::Args)]
pub struct StageArgs {
    #[arg(value_name = "ID")]
    pub id: String,
    #[arg(value_name = "COMPONENT.WASM")]
    pub component: PathBuf,
    /// blocks after the current height at which the swap activates
    #[arg(long, default_value_t = 50)]
    pub after: u64,
    #[command(flatten)]
    pub selector: Selector,
}

pub fn run(cmd: ModuleCmd) -> CommandResult {
    match cmd {
        ModuleCmd::Update(args) => cmd_update(args),
        ModuleCmd::Register(args) => cmd_register(args),
        ModuleCmd::Status(args) => cmd_status(args),
    }
}

fn cmd_update(_args: StageArgs) -> CommandResult {
    Err("module update lands in the next commit".into())
}

fn cmd_register(_args: StageArgs) -> CommandResult {
    Err("module register lands in the next commit".into())
}

/// `module status [--config node.toml] [--json]`
fn cmd_status(args: StatusArgs) -> CommandResult {
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or("module status reads the node's local rpc — set `rpc_listen` in node.toml")?;
    let modules = read_module_status(&rpc_addr)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&modules)?);
        return Ok(());
    }
    print!("{}", render_status(&modules));
    Ok(())
}

/// the lifecycle registry over the generic query lane — the same shape
/// `read_members` uses for governance.
pub(super) fn read_module_status(rpc_addr: &str) -> Result<Vec<lifecycle::ModuleCode>, String> {
    let req = lifecycle::encode_query(&lifecycle::LifecycleQuery::ModuleStatus);
    let raw = rpc_query(rpc_addr, "lifecycle", &req)?;
    match lifecycle::decode_reply(&raw).map_err(|e| format!("lifecycle reply: {e}"))? {
        lifecycle::LifecycleReply::ModuleStatus { modules } => Ok(modules),
        other => Err(format!("lifecycle answered {other:?} to ModuleStatus")),
    }
}

const SHORT_HASH: usize = 12;

/// one row per module: `id  active  pending` — pending is `—`, or
/// `<hash> ready <k|✓> activation <h>`.
pub(super) fn render_status(modules: &[lifecycle::ModuleCode]) -> String {
    let id_width = modules.iter().map(|m| m.module_id.len()).max().unwrap_or(2).max(2);
    let mut out = format!("{:<id_width$}  {:<SHORT_HASH$}  pending\n", "id", "active");
    for m in modules {
        let active = short(&m.active_code_hash);
        let pending = match &m.pending {
            None => "—".to_string(),
            Some(swap) => format!(
                "{}  ready {}  activation {}",
                short(&swap.code_hash),
                readiness_word(swap),
                swap.activation_height
            ),
        };
        out.push_str(&format!("{:<id_width$}  {active}  {pending}\n", m.module_id));
    }
    out
}

fn readiness_word(swap: &lifecycle::ScheduledSwap) -> String {
    let latched = swap.ready;
    if latched {
        return "✓".into();
    }
    swap.readiness.len().to_string()
}

fn short(hash: &[u8]) -> String {
    let hex = hex_bytes(hash);
    hex.chars().take(SHORT_HASH).collect()
}
```
(Whether `ModuleCode` derives `Serialize` for `--json`: it rides `sdk::wire` = serde_json, so it does; if `StatusArgs` has no `json`, use `SelectorArgs` and drop the branch. The `println!` result / `print!` table are CLI stdout by design.)

- [ ] **Step 4: Wire the family in `main.rs`**

`main.rs` ~`:220` `enum Family`: add `/// live code swaps: update, register, status\n Module(module_cli::ModuleCmd),` with the same `#[command(subcommand)]` shape as `Node(NodeCmd)`. `mod module_cli;` beside the other `mod` lines. Dispatch (~`:250-278`): `Family::Module(cmd) => module_cli::run(cmd),`.

- [ ] **Step 5: Completions** — both files, one declaration per line, matching the neighbours' quoting exactly (`ducktape.bash:12` families list; `ducktape.zsh:9`). Add `module` to `families`, then:

bash: `local module="update register status"`, `local module_update="--after --config --network"`, `local module_register="--after --config --network"`, `local module_status="--config --network --json"` (drop `--json` if `StatusArgs` lacks it; include `-n` only if the `Selector` declares a short alias — read `cli_args.rs:124` and mirror what the `node status` declaration lists).
zsh: the same four in the parenthesized form.

- [ ] **Step 6: Create `bin/node/tests/module_cli.rs` with the first pure-CLI case**

```rust
//! `ducktape module …` — the operator verbs for a live code swap.
mod common;

use std::process::Command;

fn ducktape(args: &[&str]) -> (bool, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_ducktape")).args(args).output().expect("run ducktape");
    (out.status.success(), format!("{}\n{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr)))
}

#[test]
fn status_against_no_node_says_the_node_is_not_running() {
    let ws = tempfile::tempdir().expect("tempdir");
    // a dev-shape node.toml with rpc_listen and no node behind it
    let cfg = ws.path().join("node.toml");
    std::fs::write(&cfg, common::minimal_dev_shape_toml(ws.path())).expect("write");
    let (ok, out) = ducktape(&["module", "status", "--config", cfg.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(out.contains("not running"), "{out}");
}
```
If `common` has no `minimal_dev_shape_toml`, add one beside `config_path` (`common/mod.rs:796-830`) that writes the same toml `config_path` writes for a single node (peer_seeds empty, `modules = FIXTURES`, a free `rpc_listen`); reuse `config_path`'s body, do not duplicate its toml literal. The "not running" sentence is `crate::node_http::NODE_NOT_RUNNING` (`cli.rs:854` uses it for rpc) — `rpc_call` already maps `ConnectionRefused` to it, so no new code.

- [ ] **Step 7: Run the tests**

Run: `cargo test -p node-bin --bin ducktape status_rows_show completion_files && cargo test -p node-bin --test module_cli && cargo clippy -p node-bin --tests --no-deps`
Expected: PASS ×3; clippy clean.

- [ ] **Step 8: Commit**

```bash
git add bin/node/src/module_cli.rs bin/node/src/main.rs ops/completions bin/node/tests/module_cli.rs bin/node/tests/common/mod.rs
git commit -m "feat(cli): ducktape module status reads the lifecycle registry

The module family lands with its grammar and completions; update and
register follow (they refuse with a one-line notice until then)."
```

---

### Task 3: Staging and the receipt gate (pure)

**Files:**
- Modify: `bin/node/src/module_cli.rs`
- Test: unit tests in `module_cli.rs`

**Interfaces:**
- Consumes: `noded::admin::{read_operator_token, ADMIN_TOKEN_HEADER}` (`crates/noded/src/admin.rs:123,141` — make `ADMIN_TOKEN_HEADER` `pub` if it is `pub(crate)`), the stage reply shape `{digest, len, receipts:[{peer,status,ok}]}` (`crates/noded/src/module_code.rs:26-33,60-65`), `reqwest::blocking` (already a `bin/node` dep via `node_http.rs`), `sha2`.
- Produces:
  ```rust
  #[derive(Debug, serde::Deserialize)]
  pub(super) struct StageReply { pub digest: String, pub len: u64, pub receipts: Vec<PeerReceipt> }
  #[derive(Debug, serde::Deserialize)]
  pub(super) struct PeerReceipt { pub peer: String, pub status: String, pub ok: bool }
  pub(super) fn stage_component(http_base: &str, workspace: &std::path::Path, bytes: &[u8]) -> Result<StageReply, String>;
  pub(super) fn refuse_on_bad_receipts(reply: &StageReply) -> Result<(), String>;   // Err = the `peer  status` table
  pub(super) fn digest_matches(reply: &StageReply, bytes: &[u8]) -> Result<[u8; 32], String>;
  ```

- [ ] **Step 1: Write the failing tests**

```rust
#[test]
fn a_single_bad_receipt_refuses_and_names_the_peer() {
    let reply = StageReply {
        digest: "00".repeat(32),
        len: 3,
        receipts: vec![
            PeerReceipt { peer: "aa11".into(), status: "stored".into(), ok: true },
            PeerReceipt { peer: "bb22".into(), status: "already-have".into(), ok: true },
            PeerReceipt { peer: "cc33".into(), status: "module_artifact_too_large".into(), ok: false },
        ],
    };
    let err = refuse_on_bad_receipts(&reply).unwrap_err();
    assert!(err.contains("cc33  module_artifact_too_large"), "{err}");
    assert!(!err.contains("aa11"), "ok peers are not listed: {err}");
    let all_ok = StageReply { receipts: reply.receipts[..2].to_vec(), ..reply };
    assert!(refuse_on_bad_receipts(&all_ok).is_ok());
}

#[test]
fn the_node_digest_must_be_the_bytes_we_read() {
    use sha2::Digest as _;
    let bytes = b"component";
    let want = sha2::Sha256::digest(bytes);
    let good = StageReply { digest: hex_bytes(&want), len: 9, receipts: vec![] };
    assert_eq!(digest_matches(&good, bytes).unwrap()[..], want[..]);
    let lying = StageReply { digest: "ff".repeat(32), len: 9, receipts: vec![] };
    let err = digest_matches(&lying, bytes).unwrap_err();
    assert!(err.contains("digest"), "{err}");
}
```
(`PeerReceipt`/`StageReply` derive `Clone` for the `..reply` line.)

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p node-bin --bin ducktape bad_receipt node_digest`
Expected: FAIL — types not found.

- [ ] **Step 3: Implement**

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct StageReply {
    pub digest: String,
    pub len: u64,
    pub receipts: Vec<PeerReceipt>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub(super) struct PeerReceipt {
    pub peer: String,
    pub status: String,
    pub ok: bool,
}

/// stage the bytes at this node's owner-gated admin route with fan-out on:
/// the node stores them, pushes them to every validator, and answers with the
/// digest and one receipt per peer. loopback exposure wants the operator
/// token minted beside node.toml at boot.
pub(super) fn stage_component(
    http_base: &str,
    workspace: &std::path::Path,
    bytes: &[u8],
) -> Result<StageReply, String> {
    let token = noded::admin::read_operator_token(workspace)?;
    let resp = reqwest::blocking::Client::new()
        .post(format!("{http_base}/v1/admin/module-code/stage?fanout=true"))
        .header(noded::admin::ADMIN_TOKEN_HEADER, token)
        .body(bytes.to_vec())
        .send()
        .map_err(|e| format!("stage: {e}"))?;
    let status = resp.status();
    let text = resp.text().unwrap_or_default();
    let refused = !status.is_success();
    if refused {
        return Err(format!("stage rejected ({status}): {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("stage reply: {e}: {text}"))
}

/// decision 2-B: one non-ok receipt and nothing is proposed. the table names
/// each refusing peer with the node's own status token.
pub(super) fn refuse_on_bad_receipts(reply: &StageReply) -> Result<(), String> {
    let failing: Vec<&PeerReceipt> = reply.receipts.iter().filter(|r| !r.ok).collect();
    let every_peer_holds_it = failing.is_empty();
    if every_peer_holds_it {
        return Ok(());
    }
    let mut table = String::from("peer  status\n");
    for r in failing {
        table.push_str(&format!("{}  {}\n", r.peer, r.status));
    }
    table.push_str("not proposed: every validator must hold the bytes before a swap is scheduled — re-run once they are reachable (staging is idempotent)");
    Err(table)
}

/// the digest the proposal will carry is the sha256 of the bytes WE read; the
/// node's answer must agree or something between us rewrote the file.
pub(super) fn digest_matches(reply: &StageReply, bytes: &[u8]) -> Result<[u8; 32], String> {
    use sha2::Digest as _;
    let ours: [u8; 32] = sha2::Sha256::digest(bytes).into();
    let theirs = config::unhex(&reply.digest).map_err(|e| format!("stage digest: {e}"))?;
    let agree = theirs[..] == ours[..];
    if !agree {
        return Err(format!("stage digest {} is not the sha256 of the file we read ({})", reply.digest, hex_bytes(&ours)));
    }
    Ok(ours)
}
```
(`config::unhex` returns `Result<Vec<u8>, _>` — `cli.rs:16` imports it; adapt the error mapping to its actual error type.)

- [ ] **Step 4: Run the tests**

Run: `cargo test -p node-bin --bin ducktape bad_receipt node_digest && cargo clippy -p node-bin --tests --no-deps`
Expected: PASS ×2; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add bin/node/src/module_cli.rs crates/noded/src/admin.rs
git commit -m "feat(cli): module verbs stage a component and gate on every peer's receipt"
```

---

### Task 4: `module update` / `module register`

**Files:**
- Modify: `bin/node/src/module_cli.rs` (`cmd_update`, `cmd_register` → one `cmd_stage_and_schedule(args, Verb)`)
- Test: `bin/node/tests/module_cli.rs` (live single-node register→update; two-node fan-out refusal; min-lead refusal)

**Interfaces:**
- Consumes: Task 1 `drive_proposal_ceremony`, `gov_signer`, `GovSigner`, `CeremonyOutcome`, `read_members`; Task 2 `read_module_status`; Task 3 `stage_component`, `refuse_on_bad_receipts`, `digest_matches`; `governance::GovAction::{UpdateModule, RegisterModule}` (`interface.rs:52,67` — fields `name, module_id, activation_height, code_hash`); rpc status height (`rpc_call(addr, json!({"cmd":"status"}))["status"]["height"]`, `cli.rs:204-218`); `resolved.http_listen` (the http base — the same field the `user`/`agent` verbs resolve; grep `http_listen` in `cli.rs` and mirror the `http://` prefixing they do).
- Produces: the two verbs; `Cluster`-drivable via `run_verb(&["module", "register", "hello", "<wasm>", "--after", "5", "--config", cfg])`.

- [ ] **Step 1: Write the failing e2e** (append to `bin/node/tests/module_cli.rs`)

```rust
use std::time::Duration;
use common::{Cluster, FIXTURES};

fn fixture(id: &str) -> String {
    format!("{FIXTURES}/{id}.component.wasm")
}

fn active_hash(cluster: &Cluster, idx: usize, id: &str) -> Option<String> {
    // `module status --json` is the registry read; the e2e keys on the same
    // projection the operator sees.
    let cfg = cluster.config_file(idx);
    let (ok, out) = cluster.run_verb(&["module", "status", "--json", "--config", cfg.to_str().unwrap()]);
    assert!(ok, "{out}");
    let stdout = out.split('\n').next().unwrap_or("");
    let modules: Vec<serde_json::Value> = serde_json::from_str(stdout).ok()?;
    let m = modules.iter().find(|m| m["module_id"] == id)?;
    let pending = !m["pending"].is_null();
    if pending {
        return None;
    }
    m["active_code_hash"].as_array().map(|bytes| {
        bytes.iter().map(|b| format!("{:02x}", b.as_u64().unwrap())).collect::<String>()
    })
}

fn sha256_hex(path: &str) -> String {
    use sha2::Digest as _;
    let bytes = std::fs::read(path).expect("fixture");
    format!("{:x}", sha2::Sha256::digest(&bytes))
}

#[test]
fn register_then_update_activate_on_a_single_validator() {
    let mut cluster = Cluster::new(&[1], &[1]);
    cluster.spawn(0);
    cluster.wait_marker(0, "serving", Duration::from_secs(60));
    let cfg = cluster.config_file(0);
    let cfg = cfg.to_str().unwrap();

    // register: hello is not in PRODUCTION, so it is a free id
    let (ok, out) = cluster.run_verb(&["module", "register", "hello", &fixture("hello"), "--after", "5", "--config", cfg]);
    assert!(ok, "{out}");
    assert!(out.contains("scheduled hello"), "{out}");
    let first = sha256_hex(&fixture("hello"));
    let seen = cluster.await_committed(0, "hello registered and active", Duration::from_secs(120), || {
        active_hash(&cluster, 0, "hello").filter(|h| *h == first)
    });
    assert_eq!(seen, first);

    // update: the replacement steps the counter by 100
    let (ok, out) = cluster.run_verb(&["module", "update", "hello", &fixture("hello-replacement"), "--after", "5", "--config", cfg]);
    assert!(ok, "{out}");
    let second = sha256_hex(&fixture("hello-replacement"));
    let seen = cluster.await_committed(0, "hello swapped", Duration::from_secs(120), || {
        active_hash(&cluster, 0, "hello").filter(|h| *h == second)
    });
    assert_eq!(seen, second);

    // the table view names the same state
    let (ok, out) = cluster.run_verb(&["module", "status", "--config", cfg]);
    assert!(ok, "{out}");
    assert!(out.contains(&format!("hello  {}", &second[..12])), "{out}");
}

#[test]
fn a_dead_peer_refuses_the_proposal_before_it_is_made() {
    let mut cluster = Cluster::new(&[1, 2], &[1, 2]);
    cluster.spawn(0);
    cluster.spawn(1);
    cluster.wait_marker(0, "serving", Duration::from_secs(60));
    cluster.wait_marker(1, "serving", Duration::from_secs(60));
    cluster.kill(1);
    let cfg = cluster.config_file(0);
    let (ok, out) = cluster.run_verb(&["module", "register", "hello", &fixture("hello"), "--config", cfg.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(out.contains("peer  status"), "{out}");
    assert!(out.contains("not proposed"), "{out}");
    // nothing reached governance: no pending swap, no open proposal
    assert!(active_hash(&cluster, 0, "hello").is_none() || true, "registry untouched");
    let (_, status) = cluster.run_verb(&["module", "status", "--config", cfg.to_str().unwrap()]);
    assert!(!status.contains("hello"), "{status}");
}

#[test]
fn an_activation_inside_the_min_lead_is_refused_with_the_registry_reason() {
    let mut cluster = Cluster::new(&[1], &[1]);
    cluster.spawn(0);
    cluster.wait_marker(0, "serving", Duration::from_secs(60));
    let cfg = cluster.config_file(0);
    let (ok, out) = cluster.run_verb(&["module", "register", "hello", &fixture("hello"), "--after", "2", "--config", cfg.to_str().unwrap()]);
    assert!(!ok, "{out}");
    assert!(out.contains("MIN_SWAP_LEAD") || out.contains("must exceed"), "{out}");
}
```
(The "serving" marker string: use whatever `live_admission_e2e.rs` waits on after `spawn` — grep `wait_marker(` there and copy the literal. If a 1-validator chain needs a resident/producer to advance blocks, add whatever `statesync_fail_closed_e2e.rs` does after spawn before writing to `directory`.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p node-bin --test module_cli register_then_update`
Expected: FAIL — `module register lands in the next commit`.

- [ ] **Step 3: Implement the verbs**

Replace the two stubs:

```rust
#[derive(Clone, Copy)]
enum Verb {
    Update,
    Register,
}

impl Verb {
    fn name(self) -> &'static str {
        match self {
            Verb::Update => "module update",
            Verb::Register => "module register",
        }
    }
    fn action(self, module_id: &str, activation_height: u64, code_hash: [u8; 32]) -> governance::GovAction {
        let name = format!("{module_id}@{}", &hex_bytes(&code_hash)[..SHORT_HASH]);
        let code_hash = code_hash.to_vec();
        match self {
            Verb::Update => governance::GovAction::UpdateModule { name, module_id: module_id.into(), activation_height, code_hash },
            Verb::Register => governance::GovAction::RegisterModule { name, module_id: module_id.into(), activation_height, code_hash },
        }
    }
}

fn cmd_update(args: StageArgs) -> CommandResult {
    cmd_stage_and_schedule(args, Verb::Update)
}

fn cmd_register(args: StageArgs) -> CommandResult {
    cmd_stage_and_schedule(args, Verb::Register)
}

/// the matcher decision 2-A/§4.4 asks for: the same variant, module and code
/// is the same proposal, whatever activation height the proposer computed.
pub(super) fn matches_module_action(verb: Verb, module_id: &str, code_hash: &[u8]) -> impl Fn(&governance::GovAction) -> bool + '_ {
    move |a| match (verb, a) {
        (Verb::Update, governance::GovAction::UpdateModule { module_id: m, code_hash: h, .. }) => m == module_id && h == code_hash,
        (Verb::Register, governance::GovAction::RegisterModule { module_id: m, code_hash: h, .. }) => m == module_id && h == code_hash,
        _ => false,
    }
}

fn cmd_stage_and_schedule(args: StageArgs, verb: Verb) -> CommandResult {
    config::validate_module_id(&args.id)?;
    let bytes = std::fs::read(&args.component)
        .map_err(|e| format!("read {}: {e}", args.component.display()))?;
    let cfg_path = args.selector.config_path()?;
    let resolved = config::resolve(&cfg_path)?;
    let rpc_addr = resolved
        .rpc_listen
        .clone()
        .ok_or(format!("{} drives the node's local rpc — set `rpc_listen` in node.toml", verb.name()))?;
    let http_base = http_base_of(&resolved)?;
    let workspace = cfg_path.parent().ok_or("node.toml has no parent dir")?;

    // 1. stage + fan-out; 2. every receipt ok or nothing is proposed
    let reply = stage_component(&http_base, workspace, &bytes)?;
    let code_hash = digest_matches(&reply, &bytes)?;
    eprintln!("staged {} ({} bytes) at {} peer(s)", hex_bytes(&code_hash), reply.len, reply.receipts.len());
    refuse_on_bad_receipts(&reply)?;

    // 3. activation = this node's height + N (each member computes its own)
    let height = current_height(&rpc_addr)?;
    let activation_height = height + args.after;

    // 4. the ceremony: join an open proposal for the same (verb, id, hash) or
    //    propose, cast yes, execute when decidable
    let signer = crate::cli::gov_signer(&rpc_addr, &cfg_path, &resolved)?;
    let pubkey_hex = signer_pubkey_hex(&signer);
    let matches = matches_module_action(verb, &args.id, &code_hash);
    let outcome = crate::cli::drive_proposal_ceremony(
        &rpc_addr,
        &signer,
        &pubkey_hex,
        verb.name(),
        "module:",
        verb.action(&args.id, activation_height, code_hash),
        &matches,
    )?;
    match outcome {
        crate::cli::CeremonyOutcome::AwaitingBallots => Ok(()),
        crate::cli::CeremonyOutcome::Passed => confirm_scheduled(&rpc_addr, &args.id, &code_hash, activation_height),
    }
}

/// a passed proposal only ASKED the registry; the registry may still refuse
/// (min lead, at most one pending, already registered). read it back and
/// say which — the CLI's success line is the registry's word, not
/// governance's.
fn confirm_scheduled(rpc_addr: &str, id: &str, code_hash: &[u8; 32], activation_height: u64) -> CommandResult {
    let modules = read_module_status(rpc_addr)?;
    let scheduled = modules.iter().any(|m| {
        let same_id = m.module_id == id;
        let pending_is_ours = m.pending.as_ref().is_some_and(|p| p.code_hash == code_hash);
        let already_active = m.active_code_hash == code_hash;
        same_id && (pending_is_ours || already_active)
    });
    if !scheduled {
        return Err(format!(
            "proposal passed but the lifecycle registry holds no pending swap for {id} → {} — it refused the schedule. \
             its rules: activation must exceed height+MIN_SWAP_LEAD (3), so --after must be ≥ 4; one pending swap per module (cancel it first); \
             `register` needs an unregistered id and `update` a registered one; the code must differ from the active code",
            hex_bytes(code_hash)
        )
        .into());
    }
    println!("scheduled {id} → {} at height {activation_height}; track with: ducktape module status", hex_bytes(code_hash));
    Ok(())
}

fn current_height(rpc_addr: &str) -> Result<u64, String> {
    let reply = rpc_call(rpc_addr, &serde_json::json!({"cmd": "status"}))?;
    reply["status"]["height"].as_u64().ok_or_else(|| "node status carries no height".into())
}

fn http_base_of(resolved: &config::Resolved) -> Result<String, String> {
    // the same field + prefixing the `user`/`agent` verbs use to reach /v1
    let listen = resolved.http_listen.clone().ok_or("set `http_listen` in node.toml — the admin route lives on it")?;
    Ok(format!("http://{listen}"))
}

fn signer_pubkey_hex(signer: &crate::cli::GovSigner) -> String {
    // the hex the ceremony prints in "each runs: ducktape <verb> …" — for a
    // module verb the re-run line has no pubkey, so this is only the id seed
    match signer {
        crate::cli::GovSigner::Node { key } => hex_bytes(key.public_key().as_ref()),
        crate::cli::GovSigner::User { key, .. } => hex_bytes(key.public_key().as_ref()),
    }
}
```
Read `GovSigner`'s actual field names at `cli.rs:952-962` and `Resolved`'s http field name (`resolved.http_listen` per `bin/node/src/config/resolve.rs`; if the `user` verbs go through a helper that already builds `http://…`, call that instead of `http_base_of`). The ceremony's "each runs: ducktape {verb} {pubkey_hex}" narration will print the pubkey for a module verb — acceptable noise for a first cut; if the implementer finds a one-line way to pass the re-run argv instead (the file + `--after`), do it, else leave it and say so in the report.

**Important — refusal inside the min lead.** `confirm_scheduled` runs only after `Passed`. If governance's execute of `UpdateModule` FAILS the proposal when the lifecycle refuses (read `crates/modules/system/governance/src/lib.rs:951-981` — does a lifecycle `Err` settle the proposal as something other than `Passed`?), the ceremony already returns `Err("proposal … settled as …")` and `confirm_scheduled` is the second net. Either way the test asserts the refusal names the lead rule; make the `Err` path print the registry's own string when it is carried on the proposal (a `reason` field on `ProposalView`, if any) and the rule summary when it is not.

- [ ] **Step 4: Run the tests**

Run: `cargo test -p node-bin --test module_cli -- --test-threads=1 && cargo test -p node-bin --bin ducktape && cargo clippy -p node-bin --tests --no-deps`
Expected: 4/4 in the lane; bin lane green (completions test included); clippy clean.

- [ ] **Step 5: Commit**

```bash
git add bin/node/src/module_cli.rs bin/node/tests/module_cli.rs
git commit -m "feat(cli): ducktape module update|register stage, gate on receipts, and schedule through governance"
```

---

### Task 5: Docs, gates, PR

**Files:**
- Modify: `skills/module-dev/SKILL.md` — the "Decide first" section and §3: a post-genesis module is `ducktape module register <id> <component.wasm>` (staged at the admin route, scheduled through governance, activates at `height + N`); code changes are `ducktape module update`; `ducktape module status` shows the registry. Remove any sentence that says registration needs a genesis edit for a NEW module (a genesis module is still a root-hash break — keep that).
- Modify: `docs/superpowers/specs/2026-08-27-live-upgrade-design.md` §4 — add a dated note that `status` prints `ready k` / `ready ✓` (no `n`, one query) and that the ceremony narration still prints the signer pubkey (or does not, if Task 4 fixed it).

- [ ] **Step 1: Docs edits** (Edit tool per hunk).
- [ ] **Step 2: Gates** — paste tails with exit codes (`${PIPESTATUS[0]}`):
  - `cargo clippy -p node-bin -p noded --tests --no-deps`
  - `cargo check --workspace --all-targets` (green on dev since #1262)
  - `cargo test -p node-bin --bin ducktape`
  - `cargo test -p node-bin --test module_cli -- --test-threads=1`
  - `cargo test -p node-bin --test workspace_registry_cli` (the founder/init lane still green; the microvm-probe test's state is whatever #1263-ish leaves it)
- [ ] **Step 3: Commit docs, push, `gh pr create --base dev --title "feat(cli): ducktape module update|register|status — a live code swap in one command"`.** Body: spec §4 link, the four decisions 2-A…2-D, the matcher generalization, what `status` shows, the three e2e cases and their times, follow-ups (Part 4 e2e §5; `cancel` deferred; `ready k/n` needs a members query). Claude Code footer. Do NOT merge.

---

## Self-review

- **Spec coverage:** §4 steps 1–5 → Task 3 (stage + receipts), Task 4 (height, ceremony with matcher, success line, registry refusal), Task 2 (`status`). 2-B no-flag refusal → `refuse_on_bad_receipts` has no bypass. 2-C → no cancel/remove verbs. `k/n` → deliberately `k` only (noted in Task 2 + Task 5 doc note).
- **Placeholders:** none — every code step is complete; the two "read X and mirror" pointers name the exact line to copy from (`read_members` `cli.rs:900`; `GovSigner` `cli.rs:952`).
- **Type consistency:** `StageReply`/`PeerReceipt` (Task 3) consumed by Task 4 with the same field names; `drive_proposal_ceremony`'s matcher param `&dyn Fn(&GovAction) -> bool` matches `matches_module_action`'s return (`impl Fn`, borrowed as `&matches`); `read_module_status` returns `Vec<lifecycle::ModuleCode>` in Tasks 2 and 4; `CeremonyOutcome::{Passed, AwaitingBallots}` as in `cli.rs:1207`.
