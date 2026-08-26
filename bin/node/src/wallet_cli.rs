//! `ducktape wallet` — cast-wallet-style porcelain over the keystore in
//! `wallet.rs` and the `user key` plumbing in `userkey_cli.rs`. Secrets
//! cross via stdin only, same as every `user key` verb.

use std::path::Path;

use keystore::wallet;

use crate::userkey_cli;

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
        assert_eq!(keystore::wallet::active_name(duck).as_deref(), Some("alice"));

        // a second new does NOT steal active.
        let mut stdin = Cursor::new("password-123\n");
        wallet_new(duck, "bob", &mut stdin).unwrap();
        assert_eq!(keystore::wallet::active_name(duck).as_deref(), Some("alice"));

        // refuse duplicates and bad names.
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(duck, "alice", &mut stdin).is_err());
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(duck, "Alice", &mut stdin).is_err());

        // use flips the pointer.
        wallet_use(duck, "bob").unwrap();
        assert_eq!(keystore::wallet::active_name(duck).as_deref(), Some("bob"));

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
