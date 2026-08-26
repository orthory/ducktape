# Wallet Keystore Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Named encrypted user keys under `~/.ducktape/keys/` with ONE active
pointer, `cast wallet`-style CLI porcelain, one shared key resolver for app
and CLI, and a wallet-first app launch flow.

**Architecture:** A new `bin/node/src/wallet.rs` keystore module (paths,
list, active pointer, legacy adoption) under a new `ducktape wallet` CLI
family; `cred` and `account-init` resolve keys through it and the
`<workspace>/user.key` rung dies. The app shells `wallet list --json` at
boot, renders a wallet list as the unlock surface (`HubStep.wallets`
replaces `HubStep.unlock`), and its `user_key_path()` default leg follows
the keystore's active pointer.

**Tech Stack:** Rust (clap CLI in `bin/node`), the ice UI language +
iced app in `app/`, bash ops scripts.

**Spec:** `docs/superpowers/specs/2026-08-26-wallet-keystore-design.md`

## Global Constraints

- No legacy/compat paths: replace old code, never keep a dual reader. The
  legacy `~/.ducktape/user.key` is MOVED into the keystore, not also read.
- Secrets (password, mnemonic) cross process boundaries via STDIN ONLY,
  one newline-delimited field per line — never argv/env.
- CLI stdout is `println!` (program output); anything else is `tracing`.
- Lint gate per touched crate: `cargo clippy -p <crate> --tests --no-deps`.
- Never `cargo fmt --all`; only format code you touched.
- Tests wait on events, never on time; env-var probes of process-global
  state run inside ONE test, sequentially.
- Gate on cargo's exit code (`${PIPESTATUS[0]}`), never on grep output.
- Edit files with the Edit tool per hunk; no sed/python edit scripts.
- Wallet names: `[a-z0-9][a-z0-9._-]{0,40}`.
- The encrypted key file format (`ducktape-user-key-v1:` prefix) is
  unchanged; a wallet file IS that format, named.

---

### Task 1: Keystore core (`wallet.rs`)

**Files:**
- Create: `bin/node/src/wallet.rs`
- Modify: `bin/node/src/main.rs` (add `mod wallet;` beside the other mods)

**Interfaces:**
- Consumes: `crate::userkey::read_user_key_file`,
  `crate::userkey::USER_KEY_ENCRYPTED_PREFIX`, `config::hex_bytes`.
- Produces (used by Tasks 2–3):
  - `pub(crate) fn duck_root() -> Result<PathBuf, String>` — `$DUCKTAPE_HOME` else `~/.ducktape`
  - `pub(crate) fn keys_dir(duck: &Path) -> PathBuf`
  - `pub(crate) fn key_file(duck: &Path, name: &str) -> PathBuf`
  - `pub(crate) fn valid_name(name: &str) -> Result<(), String>`
  - `pub(crate) fn sanitize_name(raw: &str) -> String`
  - `pub(crate) fn adopt_legacy(duck: &Path) -> Result<(), String>`
  - `pub(crate) struct WalletRow { pub name: String, pub path: PathBuf, pub pubkey: String, pub state: &'static str, pub active: bool }`
  - `pub(crate) fn list(duck: &Path) -> Result<Vec<WalletRow>, String>` (runs `adopt_legacy` first)
  - `pub(crate) fn active_name(duck: &Path) -> Option<String>`
  - `pub(crate) fn set_active(duck: &Path, name: &str) -> Result<(), String>`
  - `pub(crate) fn active_key_path(duck: &Path) -> Result<PathBuf, String>`
  - `pub(crate) fn active_user_key() -> Result<PathBuf, String>` — `$DUCKTAPE_USER_KEY` else `active_key_path(&duck_root()?)`

All core fns take `duck: &Path` so tests never mutate process env; only
`duck_root()`/`active_user_key()` read env, and the one test that covers
them runs both probes sequentially in a single `#[test]`.

- [ ] **Step 1: Write the failing tests** (bottom of `bin/node/src/wallet.rs` in `#[cfg(test)] mod tests`, using `tempfile::tempdir()` like `userkey_cli.rs` tests do):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    /// mint a real encrypted key file the way `user key init` would.
    fn seed_wallet(duck: &std::path::Path, name: &str) -> String {
        let mut seed = [0u8; 32];
        seed[0] = name.len() as u8; // distinct per name
        let line = crate::userkey::seal_user_key(&seed, "password-123").unwrap();
        let path = key_file(duck, name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        crate::userkey::write_user_key_new(&path, &line).unwrap();
        crate::config::hex_bytes(&crate::userkey::read_user_key_file(&path).unwrap().pubkey)
    }

    #[test]
    fn name_charset_is_enforced() {
        assert!(valid_name("demo").is_ok());
        assert!(valid_name("a.b-c_9").is_ok());
        assert!(valid_name("").is_err());
        assert!(valid_name("Demo").is_err());
        assert!(valid_name("../evil").is_err());
        assert!(valid_name(&"x".repeat(60)).is_err());
        assert_eq!(sanitize_name("Byeongsu Hong!"), "byeongsu-hong");
    }

    #[test]
    fn list_is_sorted_and_marks_active() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();
        seed_wallet(duck, "beta");
        let alpha_pub = seed_wallet(duck, "alpha");
        set_active(duck, "alpha").unwrap();
        let rows = list(duck).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(),
            ["alpha", "beta"]
        );
        assert!(rows[0].active && !rows[1].active);
        assert_eq!(rows[0].pubkey, alpha_pub);
        assert_eq!(rows[0].state, "encrypted");
    }

    #[test]
    fn set_active_refuses_an_unknown_name() {
        let dir = tempfile::tempdir().unwrap();
        seed_wallet(dir.path(), "alpha");
        assert!(set_active(dir.path(), "ghost").is_err());
    }

    #[test]
    fn adoption_moves_the_legacy_key_once() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();
        std::fs::create_dir_all(duck).unwrap();
        std::fs::write(duck.join("user.key"), "ducktape-user-key-v1:abc").unwrap();
        adopt_legacy(duck).unwrap();
        assert!(!duck.join("user.key").exists());
        assert!(key_file(duck, "default").exists());
        assert_eq!(active_name(duck).as_deref(), Some("default"));
        // second run is a no-op even with a new stray user.key beside a
        // populated keystore — adoption fires only when keys/ is absent.
        std::fs::write(duck.join("user.key"), "stray").unwrap();
        adopt_legacy(duck).unwrap();
        assert!(duck.join("user.key").exists());
    }

    #[test]
    fn active_key_path_errors_name_the_fix() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();
        let empty = active_key_path(duck).unwrap_err();
        assert!(empty.contains("wallet new"), "{empty}");
        seed_wallet(duck, "alpha");
        std::fs::remove_file(keys_dir(duck).join(ACTIVE_FILE)).ok();
        let dangling = active_key_path(duck).unwrap_err();
        assert!(dangling.contains("wallet use"), "{dangling}");
        set_active(duck, "alpha").unwrap();
        assert_eq!(active_key_path(duck).unwrap(), key_file(duck, "alpha"));
    }

    #[test]
    fn unreadable_file_is_listed_as_unreadable() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();
        seed_wallet(duck, "alpha");
        std::fs::write(key_file(duck, "junk"), "not a key").unwrap();
        let rows = list(duck).unwrap();
        let junk = rows.iter().find(|r| r.name == "junk").unwrap();
        assert_eq!(junk.state, "unreadable");
        assert!(junk.pubkey.is_empty());
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p node-bin wallet::tests 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: compile failure — module does not exist.

- [ ] **Step 3: Implement `bin/node/src/wallet.rs`**

```rust
//! The wallet keystore: named encrypted user keys under `<duck>/keys/`,
//! with ONE `active` pointer file naming the wallet every keyless verb
//! signs with. The file name IS the wallet name (`<name>.key`) — there is
//! no index to drift from the directory. See
//! docs/superpowers/specs/2026-08-26-wallet-keystore-design.md.

use std::path::{Path, PathBuf};

use crate::{config, userkey};

/// the active-pointer file inside `keys/` — one line, the wallet's name.
pub(crate) const ACTIVE_FILE: &str = "active";

const MAX_NAME_LEN: usize = 41;

/// `$DUCKTAPE_HOME` when set (tests, portable setups, huddle lanes), else
/// `~/.ducktape` — the directory `keys/` lives under.
pub(crate) fn duck_root() -> Result<PathBuf, String> {
    if let Some(home) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(home));
    }
    let home = std::env::var_os("HOME")
        .ok_or("cannot resolve $HOME — set DUCKTAPE_HOME or pass --key")?;
    Ok(PathBuf::from(home).join(".ducktape"))
}

pub(crate) fn keys_dir(duck: &Path) -> PathBuf {
    duck.join("keys")
}

pub(crate) fn key_file(duck: &Path, name: &str) -> PathBuf {
    keys_dir(duck).join(format!("{name}.key"))
}

/// `[a-z0-9][a-z0-9._-]{0,40}` — filesystem-safe, lowercase, never a path.
pub(crate) fn valid_name(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let head_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
    let tail_ok = chars.all(|c| {
        c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
    });
    let len_ok = name.len() <= MAX_NAME_LEN;
    if head_ok && tail_ok && len_ok {
        return Ok(());
    }
    Err(format!(
        "wallet name {name:?} is invalid — use [a-z0-9][a-z0-9._-]*, at most {MAX_NAME_LEN} chars"
    ))
}

/// Fold arbitrary display text into the wallet-name charset: lowercase,
/// runs of other characters collapse to one `-`, trimmed, truncated.
pub(crate) fn sanitize_name(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.chars() {
        let c = c.to_ascii_lowercase();
        let keep = c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-');
        match keep {
            true => out.push(c),
            false if out.ends_with('-') || out.is_empty() => {}
            false => out.push('-'),
        }
    }
    let out = out.trim_matches('-').to_string();
    let mut out = match out.is_empty() {
        true => "default".to_string(),
        false => out,
    };
    out.truncate(MAX_NAME_LEN);
    out
}

/// One-shot adoption of the pre-keystore layout: when `keys/` does not
/// exist and `<duck>/user.key` does, MOVE it in as the `default` wallet
/// and point `active` at it. A rename keeps a symlinked user.key working
/// (the link itself moves). After this, nothing reads `<duck>/user.key`
/// again — the old path is replaced, not tolerated.
pub(crate) fn adopt_legacy(duck: &Path) -> Result<(), String> {
    let keys = keys_dir(duck);
    if keys.exists() {
        return Ok(());
    }
    let legacy = duck.join("user.key");
    if !legacy.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(&keys).map_err(|e| format!("create {}: {e}", keys.display()))?;
    let target = key_file(duck, "default");
    std::fs::rename(&legacy, &target)
        .map_err(|e| format!("move {} -> {}: {e}", legacy.display(), target.display()))?;
    set_active(duck, "default")
}

/// One listed wallet. `state` is `"encrypted"` for a parseable key file and
/// `"unreadable"` for anything else (the app renders its refusal plate).
pub(crate) struct WalletRow {
    pub name: String,
    pub path: PathBuf,
    pub pubkey: String,
    pub state: &'static str,
    pub active: bool,
}

/// Every `keys/*.key`, sorted by name, with the active flag applied.
/// Runs the legacy adoption first, so the first keystore touch after an
/// upgrade converts the old layout.
pub(crate) fn list(duck: &Path) -> Result<Vec<WalletRow>, String> {
    adopt_legacy(duck)?;
    let keys = keys_dir(duck);
    let mut rows = Vec::new();
    let entries = match std::fs::read_dir(&keys) {
        Ok(entries) => entries,
        Err(_) => return Ok(rows), // no keystore yet = no wallets
    };
    let active = active_name(duck);
    for entry in entries.flatten() {
        let path = entry.path();
        let is_key_file = path.extension().is_some_and(|e| e == "key");
        if !is_key_file {
            continue;
        }
        let Some(name) = path.file_stem().and_then(|s| s.to_str()).map(String::from) else {
            continue;
        };
        let (pubkey, state) = match userkey::read_user_key_file(&path) {
            Ok(enc) => (config::hex_bytes(&enc.pubkey), "encrypted"),
            Err(_) => (String::new(), "unreadable"),
        };
        rows.push(WalletRow {
            active: active.as_deref() == Some(name.as_str()),
            name,
            path,
            pubkey,
            state,
        });
    }
    rows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(rows)
}

/// The `active` pointer's content, when present and non-empty.
pub(crate) fn active_name(duck: &Path) -> Option<String> {
    let text = std::fs::read_to_string(keys_dir(duck).join(ACTIVE_FILE)).ok()?;
    let name = text.trim();
    if name.is_empty() {
        return None;
    }
    Some(name.to_string())
}

/// Point `active` at `name` — which must exist. Atomic (tmp + rename) so a
/// concurrent reader never sees a torn pointer.
pub(crate) fn set_active(duck: &Path, name: &str) -> Result<(), String> {
    valid_name(name)?;
    if !key_file(duck, name).exists() {
        return Err(format!("no wallet named {name:?} — see `ducktape wallet list`"));
    }
    let keys = keys_dir(duck);
    std::fs::create_dir_all(&keys).map_err(|e| format!("create {}: {e}", keys.display()))?;
    let tmp = keys.join(format!("{ACTIVE_FILE}.tmp"));
    std::fs::write(&tmp, format!("{name}\n")).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, keys.join(ACTIVE_FILE)).map_err(|e| format!("activate {name}: {e}"))
}

/// The active wallet's key file, after the adoption move. The errors ARE
/// the onboarding: each names the command that fixes it.
pub(crate) fn active_key_path(duck: &Path) -> Result<PathBuf, String> {
    adopt_legacy(duck)?;
    let rows = list(duck)?;
    if rows.is_empty() {
        return Err("no wallet — run `ducktape wallet new <name>` first".into());
    }
    let Some(name) = active_name(duck) else {
        return Err("no active wallet — run `ducktape wallet use <name>`".into());
    };
    let path = key_file(duck, &name);
    if !path.exists() {
        return Err(format!(
            "active wallet {name:?} has no key file — run `ducktape wallet use <name>`"
        ));
    }
    Ok(path)
}

/// THE key resolver every keyless CLI verb signs through:
/// `$DUCKTAPE_USER_KEY` (the rig/scripted override), else the keystore's
/// active wallet.
pub(crate) fn active_user_key() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    active_key_path(&duck_root()?)
}
```

Register in `bin/node/src/main.rs` beside the existing `mod` lines: `mod wallet;`

- [ ] **Step 4: Run tests to verify pass**

Run: `cargo test -p node-bin wallet::tests 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: all 6 pass, exit=0.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p node-bin --tests --no-deps 2>&1 | tail -3
git add bin/node/src/wallet.rs bin/node/src/main.rs
git commit -m "feat(wallet): keystore core — named keys, active pointer, legacy adoption"
```

---

### Task 2: `ducktape wallet` CLI family

**Files:**
- Create: `bin/node/src/wallet_cli.rs`
- Modify: `bin/node/src/main.rs` (Family enum + dispatch, `mod wallet_cli;`)
- Modify: `bin/node/src/userkey_cli.rs` (expose two plumbing cores)

**Interfaces:**
- Consumes: Task 1's `wallet::{duck_root, key_file, valid_name, list, set_active, active_name}`;
  `userkey_cli::mint_user_key` and a new `userkey_cli::restore_user_key_at`
  (both made `pub(crate)`).
- Produces: `ducktape wallet new|import|list|use`. Print contracts:
  - `new <name>`: mnemonic line, then pubkey-hex line (pubkey LAST, the
    `last_line` convention `user key init` set).
  - `import <name>`: pubkey-hex line only. stdin: mnemonic, then password.
  - `list`: `NAME  PUBKEY  STATE  [active]` table; `list --json` emits
    `[{"name","pubkey","state","active","path"}]` — the app's data source.
  - `use <name>`: `active wallet: <name>`.

- [ ] **Step 1: Expose the plumbing cores in `userkey_cli.rs`**

Change `fn mint_user_key(` to `pub(crate) fn mint_user_key(`. Extract the
body of `user_key_restore` into a path-taking core and delegate:

```rust
/// restore core over an explicit destination — `wallet import` reuses it.
pub(crate) fn restore_user_key_at(
    out: &std::path::Path,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let mnemonic = prompt_stdin_line(stdin, "mnemonic")?;
    let password = prompt_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let seed = userkey::seed_of_mnemonic(&mnemonic)?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("restored seed is not a valid ed25519 secret: {e}"))?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

fn user_key_restore(
    args: KeyOutArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    restore_user_key_at(&args.out, stdin)
}
```

- [ ] **Step 2: Write failing tests** (in `bin/node/src/wallet_cli.rs`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn new_list_use_import_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();

        // new: mnemonic line then pubkey line; first wallet becomes active.
        let mut stdin = Cursor::new("password-123\n");
        let (words, pubkey) = wallet_new(duck, "alice", &mut stdin).unwrap();
        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(pubkey.len(), 64);
        assert_eq!(crate::wallet::active_name(duck).as_deref(), Some("alice"));

        // a second new does NOT steal active.
        let mut stdin = Cursor::new("password-123\n");
        wallet_new(duck, "bob", &mut stdin).unwrap();
        assert_eq!(crate::wallet::active_name(duck).as_deref(), Some("alice"));

        // refuse duplicates and bad names.
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(duck, "alice", &mut stdin).is_err());
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(duck, "Alice", &mut stdin).is_err());

        // use flips the pointer.
        wallet_use(duck, "bob").unwrap();
        assert_eq!(crate::wallet::active_name(duck).as_deref(), Some("bob"));

        // import round-trips the mnemonic into the SAME pubkey.
        let mut stdin = Cursor::new(format!("{words}\npassword-456\n"));
        let imported = wallet_import(duck, "alice2", &mut stdin).unwrap();
        assert_eq!(imported, pubkey);

        // list --json carries what the app needs.
        let json = wallet_list_json(duck).unwrap();
        let rows: serde_json::Value = serde_json::from_str(&json).unwrap();
        let names: Vec<&str> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["name"].as_str().unwrap())
            .collect();
        assert_eq!(names, ["alice", "alice2", "bob"]);
        assert_eq!(rows[2]["active"], serde_json::Value::Bool(true));
        assert_eq!(rows[0]["state"], "encrypted");
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p node-bin wallet_cli 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: compile failure.

- [ ] **Step 4: Implement `bin/node/src/wallet_cli.rs`**

```rust
//! `ducktape wallet` — cast-wallet-style porcelain over the keystore in
//! `wallet.rs` and the `user key` plumbing in `userkey_cli.rs`. Secrets
//! cross via stdin only, same as every `user key` verb.

use std::path::Path;

use crate::{userkey_cli, wallet};

type CommandResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum WalletCmd {
    /// mint a named wallet — stdin: password. prints mnemonic, then pubkey
    New(NameArg),
    /// restore a named wallet — stdin: mnemonic line, then password line
    Import(NameArg),
    /// list wallets (name, pubkey, state, active)
    List(ListArgs),
    /// set the active wallet every keyless verb signs with
    Use(NameArg),
}

#[derive(Debug, clap::Args)]
pub(crate) struct NameArg {
    /// the wallet name ([a-z0-9][a-z0-9._-]*)
    name: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct ListArgs {
    /// machine-readable output
    #[arg(long)]
    json: bool,
}

pub(crate) fn run(cmd: WalletCmd) -> CommandResult {
    let duck = wallet::duck_root()?;
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    match cmd {
        WalletCmd::New(args) => cmd_new(&duck, &args.name, &mut stdin),
        WalletCmd::Import(args) => cmd_import(&duck, &args.name, &mut stdin),
        WalletCmd::List(args) => cmd_list(&duck, args.json),
        WalletCmd::Use(args) => cmd_use(&duck, &args.name),
    }
}

fn cmd_new(duck: &Path, name: &str, stdin: &mut impl std::io::BufRead) -> CommandResult {
    let (words, pubkey) = wallet_new(duck, name, stdin)?;
    println!("{words}");
    println!("{pubkey}");
    Ok(())
}

/// mint core — returns (mnemonic, pubkey-hex) so tests assert both.
fn wallet_new(
    duck: &Path,
    name: &str,
    stdin: &mut impl std::io::BufRead,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let path = new_wallet_path(duck, name)?;
    let (words, key) = userkey_cli::mint_user_key(&path, stdin)?;
    activate_first_wallet(duck, name)?;
    use commonware_cryptography::Signer as _;
    Ok((words, crate::config::hex_bytes(key.public_key().as_ref())))
}

fn cmd_import(duck: &Path, name: &str, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", wallet_import(duck, name, stdin)?);
    Ok(())
}

/// import core — returns the pubkey-hex.
fn wallet_import(
    duck: &Path,
    name: &str,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let path = new_wallet_path(duck, name)?;
    let pubkey = userkey_cli::restore_user_key_at(&path, stdin)?;
    activate_first_wallet(duck, name)?;
    Ok(pubkey)
}

/// validate the name, run adoption, and refuse an occupied slot loudly —
/// `write_user_key_new` would refuse too, but with an io error instead of
/// the wallet's own vocabulary.
fn new_wallet_path(
    duck: &Path,
    name: &str,
) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
    wallet::valid_name(name)?;
    wallet::adopt_legacy(duck)?;
    let path = wallet::key_file(duck, name);
    if path.exists() {
        return Err(format!("wallet {name:?} already exists — pick another name").into());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    Ok(path)
}

/// the first wallet in an empty keystore becomes active; later mints never
/// steal the pointer.
fn activate_first_wallet(duck: &Path, name: &str) -> Result<(), String> {
    if wallet::active_name(duck).is_some() {
        return Ok(());
    }
    wallet::set_active(duck, name)
}

fn cmd_list(duck: &Path, json: bool) -> CommandResult {
    if json {
        println!("{}", wallet_list_json(duck)?);
        return Ok(());
    }
    let rows = wallet::list(duck)?;
    if rows.is_empty() {
        println!("no wallets — mint one with `ducktape wallet new <name>`");
        return Ok(());
    }
    for row in rows {
        let marker = if row.active { " [active]" } else { "" };
        let pubkey_short = row.pubkey.get(..16).unwrap_or(&row.pubkey);
        println!("{:<24} {:<18} {:<10}{marker}", row.name, pubkey_short, row.state);
    }
    Ok(())
}

fn wallet_list_json(duck: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let rows: Vec<serde_json::Value> = wallet::list(duck)?
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "name": row.name,
                "pubkey": row.pubkey,
                "state": row.state,
                "active": row.active,
                "path": row.path,
            })
        })
        .collect();
    Ok(serde_json::to_string(&rows)?)
}

fn cmd_use(duck: &Path, name: &str) -> CommandResult {
    wallet_use(duck, name)?;
    println!("active wallet: {name}");
    Ok(())
}

fn wallet_use(duck: &Path, name: &str) -> Result<(), String> {
    wallet::adopt_legacy(duck)?;
    wallet::set_active(duck, name)
}
```

In `bin/node/src/main.rs` add to `Family` (after `User`):

```rust
    /// named user-key wallets: mint, import, list, switch the active one
    #[command(subcommand)]
    Wallet(wallet_cli::WalletCmd),
```

and to the dispatch match: `Family::Wallet(cmd) => wallet_cli::run(cmd),`
plus `mod wallet_cli;`.

- [ ] **Step 5: Run tests, lint, commit**

```bash
cargo test -p node-bin wallet 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}
cargo clippy -p node-bin --tests --no-deps 2>&1 | tail -3
git add bin/node/src/wallet_cli.rs bin/node/src/main.rs bin/node/src/userkey_cli.rs
git commit -m "feat(wallet): ducktape wallet new/import/list/use porcelain"
```

---

### Task 3: CLI resolver call sites — cred + account-init

**Files:**
- Modify: `bin/node/src/cred_cli.rs:228-245` (`VerbCtx::key_path`)
- Modify: `bin/node/src/userkey_cli.rs:59-72` (AccountInitArgs help),
  `:289-350` (`cmd_user_account_init`), and the test at `:1306-1330`
- Test: existing suites in both files

**Interfaces:**
- Consumes: `wallet::{active_user_key, duck_root, key_file, sanitize_name, active_name, set_active, list, adopt_legacy}`,
  `KeyOrigin`, `load_or_mint_user_signer`.
- Produces: `wallet::account_init_target(name) -> Result<AccountInitKey>`
  where `pub(crate) enum AccountInitKey { Active(PathBuf), Mint { path: PathBuf, name: String } }`
  (add both to `wallet.rs`).

- [ ] **Step 1: Write the failing tests**

Rewrite `account_init_mints_the_user_key_when_the_workspace_has_none`
(`userkey_cli.rs:1312`) — keep its harness shape (it drives
`cmd_user_account_init` against a mock node; read it first and preserve the
mock wiring), but assert the NEW contract, and add a resolver test in
`wallet.rs::tests`:

```rust
    #[test]
    fn account_init_target_prefers_active_and_mints_only_into_emptiness() {
        let dir = tempfile::tempdir().unwrap();
        let duck = dir.path();
        // empty keystore: mint target named after --name, sanitized.
        match account_init_target_in(duck, "Byeongsu Hong").unwrap() {
            AccountInitKey::Mint { path, name } => {
                assert_eq!(name, "byeongsu-hong");
                assert_eq!(path, key_file(duck, "byeongsu-hong"));
            }
            AccountInitKey::Active(_) => panic!("empty keystore must mint"),
        }
        // populated keystore: the active wallet, never a new mint.
        seed_wallet(duck, "alice");
        set_active(duck, "alice").unwrap();
        match account_init_target_in(duck, "whoever").unwrap() {
            AccountInitKey::Active(path) => assert_eq!(path, key_file(duck, "alice")),
            AccountInitKey::Mint { .. } => panic!("populated keystore must not mint"),
        }
    }
```

(`account_init_target_in(duck, name)` is the `&Path`-taking core;
`account_init_target(name)` wraps it with `duck_root()`.)

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p node-bin account_init 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: compile failure (`AccountInitKey` missing).

- [ ] **Step 3: Implement**

In `wallet.rs`:

```rust
/// Where `user account-init` signs from when no `--key` is given: the
/// active wallet when the keystore has any; a NAMED mint only into an
/// empty keystore. Minting beside existing wallets is exactly the
/// silently-minted-stranger footgun this module exists to kill.
pub(crate) enum AccountInitKey {
    Active(PathBuf),
    Mint { path: PathBuf, name: String },
}

pub(crate) fn account_init_target(display_name: &str) -> Result<AccountInitKey, String> {
    account_init_target_in(&duck_root()?, display_name)
}

fn account_init_target_in(duck: &Path, display_name: &str) -> Result<AccountInitKey, String> {
    adopt_legacy(duck)?;
    if list(duck)?.is_empty() {
        let name = sanitize_name(display_name);
        return Ok(AccountInitKey::Mint {
            path: key_file(duck, &name),
            name,
        });
    }
    Ok(AccountInitKey::Active(active_key_path(duck)?))
}
```

In `cmd_user_account_init` (`userkey_cli.rs`), replace lines 303-306:

```rust
    // the key: explicit --key, else THE shared wallet resolver — the active
    // wallet, or (empty keystore only) a fresh wallet named after --name.
    let target = match args.key {
        Some(explicit) => crate::wallet::AccountInitKey::Active(explicit),
        None => match std::env::var_os("DUCKTAPE_USER_KEY") {
            Some(path) => crate::wallet::AccountInitKey::Active(path.into()),
            None => crate::wallet::account_init_target(&args.name)?,
        },
    };
    let (key_path, minted_wallet) = match target {
        crate::wallet::AccountInitKey::Active(path) => (path, None),
        crate::wallet::AccountInitKey::Mint { path, name } => (path, Some(name)),
    };
    let (user, origin) = load_or_mint_user_signer(&key_path, stdin)?;
    let user_pub = user.public_key();
    // the mnemonic BEFORE the submits: they take a block each, and a person
    // who ^Cs on a slow chain must still have the only copy of their seed.
    if let KeyOrigin::Minted(words) = origin {
        let wallet_name = minted_wallet.as_deref().unwrap_or("(explicit path)");
        if let Some(name) = &minted_wallet {
            crate::wallet::set_active(&crate::wallet::duck_root()?, name)?;
        }
        println!("a new wallet {wallet_name:?} was minted at {}", key_path.display());
        println!("write these 24 words down — they are the only way to restore it:");
        println!("{words}");
    }
```

Update `AccountInitArgs`'s `--key` help text to:
`/// path to the user key file (defaults to the active wallet, minting one named after --name into an empty keystore)`

In `cred_cli.rs` replace `VerbCtx::key_path`'s `None` arm and the error:

```rust
    /// the user key path for the signing verbs: explicit `--key` wins, else
    /// THE shared wallet resolver ($DUCKTAPE_USER_KEY, else the keystore's
    /// active wallet). A MISSING key is a loud error, never a cue to mint:
    /// cred always signs as an already-bound account.
    fn key_path(&self) -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let path = match &self.key {
            Some(explicit) => explicit.clone(),
            None => crate::wallet::active_user_key()?,
        };
        if !path.exists() {
            return Err(format!(
                "no user key at {} — run `ducktape user account-init --name <you>` first \
                 (it mints one), or pass --key",
                path.display()
            )
            .into());
        }
        Ok(path)
    }
```

- [ ] **Step 4: Run the full node-bin test suite**

Run: `cargo test -p node-bin 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: pass. The old account-init test now asserts: empty keystore →
wallet minted at `keys/<sanitized-name>.key`, `active` points at it, and
the workspace directory gained NO `user.key`.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p node-bin --tests --no-deps 2>&1 | tail -3
git add bin/node/src/wallet.rs bin/node/src/cred_cli.rs bin/node/src/userkey_cli.rs
git commit -m "feat(wallet): cred + account-init resolve through the keystore, workspace rung dies"
```

---

### Task 4: App backend — keystore-aware key path + wallet externs

**Files:**
- Modify: `app/src/backend/rpc.rs:524-535` (`user_key_path`), `:390-397`
  (`prefs_path` — mechanical: reuse the new `duck_home()`)
- Modify: `app/src/backend/hub.rs` (`HubState`, `hub_state`,
  `hub_entry_step`, `user_key_state` stays; new `WalletInfo`,
  `wallet_rows`, `unlock_wallet`, `use_wallet`; `create_user_key` /
  `restore_user_key` re-target `wallet new` / `wallet import`)
- Test: inline `#[cfg(test)]` in `hub.rs` (pure fns only)

**Interfaces:**
- Consumes: `ducktape wallet list --json` / `new` / `import` / `use`
  (Task 2's print contracts), `user_key_cli` / `user_key_cli_raw`
  (`hub.rs:482-534`), `set_local_user_key` (`rpc.rs:340`).
- Produces (Task 5's extern surface):
  - `pub struct WalletInfo { pub name: String, pub pubkey: String, pub state: String, pub active: bool }`
  - `HubState` gains `pub wallets: Vec<WalletInfo>`
  - `pub fn hub_entry_step(wallets: Vec<WalletInfo>) -> crate::HubStep`
  - `pub fn preselect_wallet(wallets: Vec<WalletInfo>) -> String`
  - `pub async fn unlock_wallet(name: String, password: String) -> Result<String, AppError>`
  - `pub async fn create_user_key(name: String, password: String) -> Result<KeyCreated, AppError>`
  - `pub async fn restore_user_key(name: String, words: SecretText, password: String) -> Result<String, AppError>`
  - the env-bypass wallet row is named `"env"`.

- [ ] **Step 1: Write failing tests for the pure pieces** (in `hub.rs`'s test mod; the subprocess fns are covered by Task 5's ice tests and the qa lane):

```rust
    #[test]
    fn entry_step_and_preselect_follow_the_keystore() {
        let rows = |actives: &[(&str, bool)]| -> Vec<WalletInfo> {
            actives
                .iter()
                .map(|(name, active)| WalletInfo {
                    name: name.to_string(),
                    pubkey: String::new(),
                    state: "encrypted".into(),
                    active: *active,
                })
                .collect()
        };
        assert!(matches!(hub_entry_step(vec![]), crate::HubStep::Create));
        assert!(matches!(
            hub_entry_step(rows(&[("a", false)])),
            crate::HubStep::Wallets
        ));
        assert_eq!(preselect_wallet(rows(&[("a", false), ("b", true)])), "b");
        assert_eq!(preselect_wallet(rows(&[("a", false)])), "a");
        assert_eq!(preselect_wallet(vec![]), "");
    }

    #[test]
    fn wallet_rows_parse_the_list_json() {
        let json = r#"[{"name":"demo","pubkey":"ab","state":"encrypted","active":true,"path":"/x/demo.key"}]"#;
        let rows = parse_wallet_rows(json).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "demo");
        assert!(rows[0].active);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p ducktape-app hub 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: compile failure. (`crate::HubStep::Wallets` also fails until
Task 5's ice enum change — Tasks 4 and 5 compile TOGETHER; write both
before the first green build, but keep the commits separate.)

- [ ] **Step 3: Implement the backend**

`rpc.rs` — replace `user_key_path` and give `prefs_path` the same root
(label the `prefs_path` change as the mechanical refactor it is):

```rust
/// `$DUCKTAPE_HOME` else `~/.ducktape` — where the keystore and prefs live.
fn duck_home() -> Result<PathBuf, String> {
    if let Some(root) = std::env::var_os("DUCKTAPE_HOME") {
        return Ok(PathBuf::from(root));
    }
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".ducktape"))
        .ok_or_else(|| "cannot locate ~/.ducktape; set DUCKTAPE_USER_KEY".to_string())
}

pub(crate) fn user_key_path() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("DUCKTAPE_USER_KEY") {
        return Ok(path.into());
    }
    let keys = duck_home()?.join("keys");
    let pointer = keys.join("active");
    let name = std::fs::read_to_string(&pointer)
        .map(|text| text.trim().to_string())
        .unwrap_or_default();
    if name.is_empty() {
        return Err("no active wallet — pick one in the launch window".to_string());
    }
    Ok(keys.join(format!("{name}.key")))
}

fn prefs_path() -> Option<PathBuf> {
    duck_home().ok().map(|home| home.join("app-prefs.json"))
}
```

`hub.rs` — the wallet surface (place beside `HubState`):

```rust
/// One wallet row the launch window lists — `wallet list --json`, verbatim.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct WalletInfo {
    pub name: String,
    pub pubkey: String,
    pub state: String,
    pub active: bool,
}

pub(crate) fn parse_wallet_rows(json: &str) -> Option<Vec<WalletInfo>> {
    serde_json::from_str(json).ok()
}

/// the synthetic row a `DUCKTAPE_USER_KEY` override renders as.
const ENV_WALLET: &str = "env";

/// The keystore's rows. `DUCKTAPE_USER_KEY` bypasses the keystore with one
/// synthetic row so rigs and huddle lanes get the same single screen. A
/// failed/missing CLI degrades to an empty list — the create screen's own
/// `wallet new` will then surface the real error with its own message.
async fn wallet_rows() -> Vec<WalletInfo> {
    if std::env::var_os("DUCKTAPE_USER_KEY").is_some() {
        return vec![WalletInfo {
            name: ENV_WALLET.into(),
            pubkey: String::new(),
            state: user_key_state(),
            active: true,
        }];
    }
    let mut command = tokio::process::Command::new(ducktape_binary());
    command
        .arg("wallet")
        .arg("list")
        .arg("--json")
        .kill_on_drop(true);
    let Ok(Ok(output)) = tokio::time::timeout(CLI_TIMEOUT, command.output()).await else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }
    std::str::from_utf8(&output.stdout)
        .ok()
        .and_then(parse_wallet_rows)
        .unwrap_or_default()
}

/// no wallets on disk means the create ceremony; anything else lands on
/// the wallet list — the unlock surface.
pub fn hub_entry_step(wallets: Vec<WalletInfo>) -> crate::HubStep {
    if wallets.is_empty() {
        crate::HubStep::Create
    } else {
        crate::HubStep::Wallets
    }
}

/// the row the list preselects: the active wallet, else the first.
pub fn preselect_wallet(wallets: Vec<WalletInfo>) -> String {
    wallets
        .iter()
        .find(|row| row.active)
        .or_else(|| wallets.first())
        .map(|row| row.name.clone())
        .unwrap_or_default()
}

/// The named wallet's key file — the env row unlocks the override path.
fn wallet_key_path(name: &str) -> Result<PathBuf, String> {
    if name == ENV_WALLET {
        return user_key_path();
    }
    let root = std::env::var_os("DUCKTAPE_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".ducktape"))
        })
        .ok_or_else(|| "cannot locate ~/.ducktape".to_string())?;
    Ok(root.join("keys").join(format!("{name}.key")))
}

/// Unlock the NAMED wallet, persist it as active, seed the identity cache.
pub async fn unlock_wallet(name: String, password: String) -> Result<String, AppError> {
    let path = wallet_key_path(&name).map_err(AppError::from)?;
    let stdout = user_key_cli(&["unlock", "--key"], &path, password).await?;
    let pubkey = last_line(&stdout)?;
    if name != ENV_WALLET {
        let mut command = tokio::process::Command::new(ducktape_binary());
        command.arg("wallet").arg("use").arg(&name).kill_on_drop(true);
        let output = tokio::time::timeout(CLI_TIMEOUT, command.output())
            .await
            .map_err(|_| AppError::from("wallet use timed out".to_string()))?
            .map_err(|error| AppError::from(error.to_string()))?;
        if !output.status.success() {
            return Err(AppError::from(bounded_detail(&output.stderr)));
        }
    }
    set_local_user_key(hex_decode(&pubkey).ok()).await;
    Ok(pubkey)
}
```

Rework `hub_state()` — the `--version` prewarm spawn becomes the awaited
`wallet_rows()` call (one subprocess, same Gatekeeper warm, plus it runs
the keystore adoption on a first post-upgrade boot); `HubState` gains
`wallets`. Re-target the ceremonies: `create_user_key(name, password)`
shells `["wallet", "new", &name]` via a generalized raw helper (extend
`user_key_cli_raw` to take the full argv slice instead of prefixing
`user key`; its three existing callers pass `["user", "key", "init", "--out"]`
etc. — mechanical, label it), parses the words line + pubkey line into
`KeyCreated`; `restore_user_key(name, words, password)` shells
`["wallet", "import", &name]` with the two stdin lines. Both end with
`set_local_user_key` exactly as today. `unlock_user_key(password)` (the
Settings re-unlock) keeps its signature, delegating to
`unlock_wallet(active_or_env_name(), password)` where
`active_or_env_name()` returns `ENV_WALLET` under the override, else the
active pointer's name, else an error.

Match the existing signatures for `AppError` conversion, `last_line`,
`bounded_detail`, `hex_decode` by reading their current uses in
`hub.rs:386-451` before writing.

- [ ] **Step 4: Commit** (compile lands with Task 5)

```bash
git add app/src/backend/hub.rs app/src/backend/rpc.rs
git commit -m "feat(app): keystore-aware key path + wallet backend externs"
```

---

### Task 5: App UI — wallet-first launch flow

**Files:**
- Modify: `app/src/ui/state/types.ice:50-59` (`HubStep`: `unlock` → `wallets`)
- Modify: `app/src/ui/state/onboarding.ice` (add `hub_wallets`, `hub_wallet_selected`)
- Modify: `app/src/ui/extern/backend.ice:146-160` (WalletInfo, HubState,
  new/changed externs)
- Modify: `app/src/ui/handlers/onboarding.ice` (boot, unlock, create,
  restore, go_wallets)
- Modify: `app/src/ui/components/onboarding.ice` (WalletsScreen +
  WalletRow new; UnlockScreen deleted; HubColumn arm swap; CreateScreen /
  RestoreScreen name inputs; NetworksScreen switch-wallet affordance)
- Modify: `app/src/ui/view.ice` (HubColumn props/events)
- Test: `app/src/ui/tests/app.ice`

**Interfaces:**
- Consumes: Task 4's backend surface exactly as named there.
- Produces: emissions `pick_wallet(str)`, `go_wallets`; changed emissions
  `unlock_submit(str)` (unchanged shape, new meaning: unlock the SELECTED
  wallet), `create_submit(str, str)` (name, password),
  `restore_submit(str, str)` (name, password).

- [ ] **Step 1: Write the failing ice test** (append to `app/src/ui/tests/app.ice`):

```
preset ui_wallets
  state
    mutation_phase = MutationPhase.idle
    onboarding_error = ""
    hub_wallets = [WalletInfo("alice", "aabbccdd", "encrypted", false), WalletInfo("demo", "eeff0011", "encrypted", true)]
    hub_wallet_selected = "demo"

test wallet_list_contract
  preset ui_wallets
  viewport 520 680
  mount
    WalletsScreen #wallets
      with
        wallets=hub_wallets
        selected=hub_wallet_selected
        busy=false
        error=""
  // the active wallet is preselected: its row shows the password input.
  expect exists wallets
  expect exists wallet-password
  expect text "demo" within wallets
  expect text "alice" within wallets
  // selecting the other row moves the input there.
  click wallet-row-alice
  expect hub_wallet_selected == "alice"
  type "hunter2-hunter2"
  expect wallet-password.value == "hunter2-hunter2"
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p ducktape-app 2>&1 | tail -5; echo exit=${PIPESTATUS[0]}`
Expected: FAIL — `WalletsScreen` unknown.

- [ ] **Step 3: Implement the ice changes**

`state/types.ice` — in `enum HubStep` replace the `unlock` line with
`wallets`.

`state/onboarding.ice` — after `hub_hidden:i64 = 0` add:

```
  hub_wallets:[WalletInfo] = []
  hub_wallet_selected = ""
```

`extern/backend.ice` — in the hub block:

```
  WalletInfo(name:str, pubkey:str, state:str, active:bool)
  HubState(key_state:str, wallets:[WalletInfo], networks:[HubNetwork], preselect:str, hidden:i64)
  pure hub_entry_step(wallets:[WalletInfo]) -> HubStep
  pure preselect_wallet(wallets:[WalletInfo]) -> str
  unlock_wallet(name:str, password:str) -> str ! AppError
  create_user_key(name:str, password:str) -> KeyCreated ! AppError
  restore_user_key(name:str, words:secret, password:str) -> str ! AppError
```

(replacing the old `hub_entry_step(key_state:str)`, `create_user_key(password:str)`,
`restore_user_key(words:secret, password:str)`, `unlock_user_key(password:str)`
lines; `unlock_user_key(password:str) -> str ! AppError` STAYS for the
Settings re-unlock in `handlers/node.ice`.)

`handlers/onboarding.ice`:

```
on hub_booted(state)
  hub_key_state = state.key_state
  hub_hidden = state.hidden
  hub_networks = state.networks
  hub_selected = state.preselect
  hub_wallets = state.wallets
  hub_wallet_selected = preselect_wallet(state.wallets)
  hub_step = hub_entry_step(state.wallets)
  stream replace lane=network_probes probe_known_networks() -> network_probed _

on pick_wallet(name)
  hub_wallet_selected = name

// UNLOCK — verify the password opens the SELECTED wallet, persist it as
// the active wallet, keep the password as the session's signing password.
on unlock_submit(pw)
  return if mutation_phase != MutationPhase.idle || empty(pw) || empty(hub_wallet_selected)
  onboarding_error = ""
  password = pw
  mutation_phase = MutationPhase.onboarding
  run every unlock_wallet(hub_wallet_selected, password) -> key_unlocked _ | login_failed _

on go_wallets
  hub_step = HubStep.wallets
```

(`key_unlocked` / `login_skip` keep their bodies; `go_login`'s
`hub_step = HubStep.unlock` assignments — there are two, in `go_login` and
after restore — become `HubStep.wallets`. `create_submit(pw)` becomes
`create_submit(name, pw)` calling `create_user_key(name, password)`;
`restore_submit(pw)` likewise threads its name into
`restore_user_key(name, restore_words, password)`. Read each existing
handler and keep its `mutation_phase` / error discipline identical.)

`components/onboarding.ice` — DELETE `UnlockScreen`; ADD (modeled
byte-for-byte on `NetworksScreen`/`NetworkRow`/`UnlockScreen` styling):

```
component WalletsScreen(wallets:[WalletInfo], selected:str, busy:bool, error:str)
  emits
    pick_wallet(str)
    unlock_submit(str)
    login_skip
    go_restore
  col #root w=428.0 gap=0.0
    HubBrand title="Choose a wallet" caption="Unlock an identity to sign what you do."
    box w=fill pt=22.0
      scroll dir=vertical h=300.0
        col w=fill gap=8.0
          for row in wallets
            WalletRow #wallet-row-{row.name}
              with
                row
                selected=(row.name == selected)
                busy
              forward
                pick_wallet
                unlock_submit
    box w=fill pt=18.0
      col
        with
          w=fill
          gap=8.0
          align=center
        button "Restore from recovery phrase" -> emit(go_restore)
          with
            disabled=busy
            h=26.0
            p=5.0
            @ghost_action
          active bg=transparent text=muted r=7.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
        button "Continue read-only" -> emit(login_skip)
          with
            disabled=busy
            h=26.0
            p=5.0
            @ghost_action
          active bg=transparent text=muted r=7.0
          hovered bg=fg/9 text=fg
          pressed bg=fg/14
    OnboardingError message=error

component WalletRow(row:WalletInfo, selected:bool, busy:bool)
  emits
    pick_wallet(str)
    unlock_submit(str)
  state
    pw = ""
  col #root w=fill gap=0.0
    if selected
      col w=fill gap=0.0
        button -> emit(pick_wallet, row.name)
          with
            label=row.name
            checked=selected
            w=fill
            p=0.0
            @icon_action
          box
            with
              w=fill
              px=13.0
              pt=11.0
              pb=11.0
            col w=fill gap=4.0
              row w=fill gap=9.0 align=center
                text row.name
                  with
                    w=fill
                    size=13.5
                    wrap=none
                    font=display
                    @text-primary
                if row.active
                  text "active"
                    with
                      size=9.5
                      wrap=none
                      font=code_semibold
                      @text-label
              text short_pubkey(row.pubkey)
                with
                  w=fill
                  size=11.0
                  wrap=none
                  font=code_medium
                  @text-meta
          active bg=selected_row text=fg border=primary border-w=1.5 r=11.0
          hovered bg=selected_row text=fg
          pressed bg=rail_hover text=fg
        if row.state == "encrypted"
          box w=fill pt=8.0
            box
              with
                w=fill
                px=14.0
                py=12.0
                bg=surface
                border=primary
                border-w=1.5
                r=10.0
              input "" #wallet-password <-> pw
                with
                  label="Key password"
                  hint="••••••••"
                  secure=true
                  disabled=busy
                  submit=emit(unlock_submit, pw)
                  w=fill
                  p=0.0
                  text-size=13.0
                  line-h=1.2
                  font=code
                  @control
                active bg=transparent border=transparent value=fg placeholder=label selection=fg/18 border-w=0.0 r=0.0
                disabled value=hint
          box w=fill pt=10.0
            button -> emit(unlock_submit, pw)
              with
                label="Unlock"
                disabled=(busy || empty(pw))
                w=fill
                @primary_action
                @px-0px
                @py-13px
                @rounded-10px
              text "Unlock →"
                with
                  w=fill
                  size=13.5
                  wrap=none
                  align-x=center
                  font=display
                  @text-primary_fg
        if row.state != "encrypted"
          box w=fill pt=8.0
            GateNote
              with
                reason="This wallet's key file is not usable for signing."
                next="`ducktape user key status` explains; restore from the recovery phrase or continue read-only."
    if !selected
      button -> emit(pick_wallet, row.name)
        with
          label=row.name
          checked=selected
          w=fill
          p=0.0
          @icon_action
        box
          with
            w=fill
            px=13.0
            pt=11.0
            pb=11.0
          col w=fill gap=4.0
            row w=fill gap=9.0 align=center
              text row.name
                with
                  w=fill
                  size=13.5
                  wrap=none
                  font=display
                  @text-primary
              if row.active
                text "active"
                  with
                    size=9.5
                    wrap=none
                    font=code_semibold
                    @text-label
            text short_pubkey(row.pubkey)
              with
                w=fill
                size=11.0
                wrap=none
                font=code_medium
                @text-meta
        active bg=surface text=muted border=border border-w=1.0 r=11.0
        hovered bg=subtle text=fg
        pressed bg=rail_hover text=fg
```

(`short_pubkey` is a tiny new `pure` extern in `hub.rs` + `backend.ice`:
first 16 hex chars + `…`, empty in → empty out. No string concat in ice —
Rust helpers, per the tray-menu rule. The `#wallet-row-{row.name}` id form:
check how existing per-row ids are minted in this file — if interpolated
ids are not a thing, give the row buttons plain `#wallet-row` ids and
drive selection in the test via `click` on text instead.)

`HubColumn`: replace the `HubStep.unlock` arm with:

```
        HubStep.wallets
          WalletsScreen #wallets
            with
              wallets
              wallet_selected
              busy
              error
            forward
              pick_wallet
              unlock_submit
              login_skip
              go_restore
```

adding `wallets:[WalletInfo]` and `wallet_selected:str` to HubColumn's
props and `pick_wallet(str)` / `go_wallets` to its emits (drop nothing
else). `CreateScreen` and `RestoreScreen` gain a name input above their
password fields (same bordered-box input pattern, ids `#wallet-name` /
`#restore-name`, prefilled via `state name_draft = "default"` /
`"restored"`) and emit `create_submit(name_draft, pw)` /
`restore_submit(name_draft, pw)`. `NetworksScreen` gains an
`active_wallet:str` prop rendered as a one-line
`text active_wallet_label(active_wallet)` + ghost button
`"Switch wallet" -> emit(go_wallets)` above the rows
(`active_wallet_label` is another tiny pure extern; HubColumn passes
`active_wallet=wallet_selected`).

`view.ice`: add `wallets=hub_wallets`, `wallet_selected=hub_wallet_selected`
to the HubColumn `with` block; add `pick_wallet -> pick_wallet _` and
`go_wallets -> go_wallets` to `events`; change
`create_submit -> create_submit _` to `create_submit -> create_submit _ _`
and `restore_submit -> restore_submit _ _` likewise.

- [ ] **Step 4: Build + run the app suite**

Run: `cargo test -p ducktape-app 2>&1 | tail -8; echo exit=${PIPESTATUS[0]}`
Expected: pass, including `wallet_list_contract`. Fix ice syntax errors by
reading the codegen error, not by guessing (each build round-trip is
expensive). The pre-existing onboarding tests that mounted `UnlockScreen`
(if any exist — grep `app/src/ui/tests/` first) are rewritten to
`WalletsScreen`.

- [ ] **Step 5: Lint and commit**

```bash
cargo clippy -p ducktape-app --tests --no-deps 2>&1 | tail -3
git add app/src
git commit -m "feat(app): wallet-first launch — list, per-row unlock, switch affordance"
```

---

### Task 6: Ops scripts — demo-seed wallet + huddle-lane override

**Files:**
- Modify: `ops/demo-seed.sh:26-27, 97-112, 204-247` (USERKEY, section 3b,
  gateway password, banner)
- Modify: `ops/huddle-lane.sh:151-175` (per-side key env)

- [ ] **Step 1: demo-seed.sh**

Replace `USERKEY="$DUCK/user.key"` with `USERKEY="$DUCK/keys/demo.key"`.
Replace section 3b's provisioning block with:

```bash
# ── 3b. user identity ──────────────────────────────────────────
# The app signs writes with a wallet from the keystore. The demo gets its
# OWN named wallet ("demo", password $DEMO_PASSWORD) so the seed always
# holds the signing password: the old "existing key, unknown password,
# routes skipped" branch cannot happen. The user's other wallets are
# untouched; the seed never flips the active pointer — the app's wallet
# list is where the demo identity gets picked.
if [ -e "$USERKEY" ]; then
  log "demo wallet already present at $USERKEY"
else
  printf '%s\n' "$DEMO_PASSWORD" | "$NODE_BIN" wallet new demo >/dev/null \
    || die "could not mint the demo wallet"
  log "minted the demo wallet (password: $DEMO_PASSWORD)"
fi
```

Set `GATEWAY_PW="$DEMO_PASSWORD"` unconditionally, delete the
`KEY_PROVISIONED` variable and both of its `case`/banner branches (the
skip-reason log line and the two closing-banner variants collapse to the
one provisioned wording, which now says
`key password: $DEMO_PASSWORD   (the "demo" wallet — pick it in the app's wallet list)`).

- [ ] **Step 2: huddle-lane.sh**

Keep the per-side mint (`user key init --out "$LANE/home-$side/user.key"`
still works — plumbing is path-explicit). In the two app-launch env blocks
(`:166`, `:173`) add `DUCKTAPE_USER_KEY=$LANE/home-<side>/user.key \\`
above the existing `DUCKTAPE_HOME` line so the app bypasses the wallet
list with the synthetic env row.

- [ ] **Step 3: Verify by seed**

```bash
DEMO_WORKSPACE_ID=walletcheck DUCKTAPE_HOME=$(mktemp -d) bash ops/demo-seed.sh 2>&1 | tail -15
```

Expected: seeds clean; the gateway-routes step reports 3 routes (never the
skipped-password branch); `$DUCKTAPE_HOME/keys/demo.key` exists. Remove the
temp dir afterwards.

- [ ] **Step 4: Commit**

```bash
git add ops/demo-seed.sh ops/huddle-lane.sh
git commit -m "feat(wallet): demo-seed mints the demo wallet; huddle lanes pin DUCKTAPE_USER_KEY"
```

---

### Task 7: Gates, live check, PR

- [ ] **Step 1: Full gates**

```bash
cargo clippy -p node-bin --tests --no-deps 2>&1 | tail -3
cargo clippy -p ducktape-app --tests --no-deps 2>&1 | tail -3
cargo test -p node-bin 2>&1 | tail -3; echo exit=${PIPESTATUS[0]}
cargo test -p ducktape-app 2>&1 | tail -3; echo exit=${PIPESTATUS[0]}
cargo check -p files --no-default-features 2>&1 | tail -2
```

All green (the files gate is untouched by this work and must stay green).

- [ ] **Step 2: CLI smoke against a scratch home**

```bash
H=$(mktemp -d)
printf 'password-123\n' | DUCKTAPE_HOME=$H target/debug/ducktape wallet new alice
DUCKTAPE_HOME=$H target/debug/ducktape wallet list
printf 'password-456\n' | DUCKTAPE_HOME=$H target/debug/ducktape wallet new bob
DUCKTAPE_HOME=$H target/debug/ducktape wallet use bob
DUCKTAPE_HOME=$H target/debug/ducktape wallet list --json
rm -rf $H
```

Expected: alice active after first mint, bob after `use`; JSON rows carry
name/pubkey/state/active/path.

- [ ] **Step 3: Push and open the PR against dev**

```bash
git push -u origin feat/wallet-keystore
gh pr create --base dev --title "wallet keystore: named user keys, one active pointer, wallet-first app entry" --body "..."
```

PR body: the spec path, the failure chain this kills (app/CLI key-path
split → account-init minting a stranger → 'already bound'), the surfaces
touched (bin/node keystore + wallet family + cred/account-init resolver,
app wallet-first launch, demo-seed/huddle-lane), and the gates run.
End the body with the standard generated-with footer.
