//! `ducktape wallet` — cast-wallet-style porcelain over the keystore in
//! `wallet.rs` and the `user key` plumbing in `userkey_cli.rs`. Secrets
//! cross via stdin only, same as every `user key` verb.
//!
//! The keystore is the WORKSPACE's (`<workspace>/keys/`): a wallet is an
//! identity on one network, so the verb group carries the workspace selector
//! every file-editing family carries.

use std::path::Path;

use keystore::wallet;

use crate::cli_args::WorkspaceArgs;
use crate::userkey_cli;

type CommandResult = Result<(), Box<dyn std::error::Error>>;

#[derive(Debug, clap::Args)]
pub(crate) struct WalletArgs {
    #[command(flatten)]
    workspace: WorkspaceArgs,
    #[command(subcommand)]
    cmd: WalletCmd,
}

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

pub(crate) fn run(args: WalletArgs) -> CommandResult {
    let workspace = args.workspace.dir()?;
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    match args.cmd {
        WalletCmd::New(name) => cmd_new(&workspace, &name.name, &mut stdin),
        WalletCmd::Import(name) => cmd_import(&workspace, &name.name, &mut stdin),
        WalletCmd::List(list) => cmd_list(&workspace, list.json),
        WalletCmd::Use(name) => cmd_use(&workspace, &name.name),
    }
}

fn cmd_new(workspace: &Path, name: &str, stdin: &mut impl std::io::BufRead) -> CommandResult {
    let (words, pubkey, activated) = wallet_new(workspace, name, stdin)?;
    println!("{words}");
    println!("{pubkey}");
    if !activated {
        println!("wallet minted but not activated — run `ducktape wallet use {name}`");
    }
    Ok(())
}

/// mint core — returns (mnemonic, pubkey-hex, activated) so tests assert all
/// three; `activated` is false only when the mint succeeded but the pointer
/// write did not (see `wallet::create`'s doc comment).
fn wallet_new(
    workspace: &Path,
    name: &str,
    stdin: &mut impl std::io::BufRead,
) -> Result<(String, String, bool), Box<dyn std::error::Error>> {
    let password = userkey_cli::prompt_stdin_line(stdin, "password")?;
    Ok(wallet::create(workspace, name, &password)?)
}

fn cmd_import(workspace: &Path, name: &str, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", wallet_import(workspace, name, stdin)?);
    Ok(())
}

/// import core — returns the pubkey-hex.
fn wallet_import(
    workspace: &Path,
    name: &str,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let mnemonic = userkey_cli::prompt_stdin_line(stdin, "mnemonic")?;
    let password = userkey_cli::prompt_stdin_line(stdin, "password")?;
    Ok(wallet::import(workspace, name, &mnemonic, &password)?)
}

fn cmd_list(workspace: &Path, json: bool) -> CommandResult {
    if json {
        println!("{}", wallet_list_json(workspace)?);
        return Ok(());
    }
    let rows = wallet::list(workspace)?;
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

fn wallet_list_json(workspace: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let rows: Vec<serde_json::Value> = wallet::list(workspace)?
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

fn cmd_use(workspace: &Path, name: &str) -> CommandResult {
    wallet_use(workspace, name)?;
    println!("active wallet: {name}");
    Ok(())
}

fn wallet_use(workspace: &Path, name: &str) -> Result<(), String> {
    wallet::activate(workspace, name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn new_list_use_import_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path();

        // new: mnemonic line then pubkey line; first wallet becomes active.
        let mut stdin = Cursor::new("password-123\n");
        let (words, pubkey, activated) = wallet_new(workspace, "alice", &mut stdin).unwrap();
        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(pubkey.len(), 64);
        assert!(activated);
        assert_eq!(
            keystore::wallet::active_name(workspace).as_deref(),
            Some("alice")
        );

        // a second new does NOT steal active.
        let mut stdin = Cursor::new("password-123\n");
        wallet_new(workspace, "bob", &mut stdin).unwrap();
        assert_eq!(
            keystore::wallet::active_name(workspace).as_deref(),
            Some("alice")
        );

        // refuse duplicates and bad names.
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(workspace, "alice", &mut stdin).is_err());
        let mut stdin = Cursor::new("password-123\n");
        assert!(wallet_new(workspace, "Alice", &mut stdin).is_err());

        // use flips the pointer.
        wallet_use(workspace, "bob").unwrap();
        assert_eq!(
            keystore::wallet::active_name(workspace).as_deref(),
            Some("bob")
        );

        // import round-trips the mnemonic into the SAME pubkey.
        let mut stdin = Cursor::new(format!("{words}\npassword-456\n"));
        let imported = wallet_import(workspace, "alice2", &mut stdin).unwrap();
        assert_eq!(imported, pubkey);

        // list --json carries what the app needs.
        let json = wallet_list_json(workspace).unwrap();
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
