//! `ducktape account` — the ACCOUNT this user key belongs to: a number-keyed
//! principal (`identity` module) that a set of keys of mixed schemes associate
//! under. No node is ever bound to an account; attribution comes from a
//! user-signed origin, so every write here is ONE frame the active wallet key
//! signs, POSTed to `/v1/submit/frame` ([`crate::node_http::submit_frame`]).
//!
//! - `create --name <s>` founds an account for the active key.
//! - `show [--number N | --pubkey <hex>]` reads one (default: this key's).
//! - `key list|approve|add|join|remove` — admitting a second device is a
//!   two-sided ceremony: the NEW device prints its key (`approve`), an EXISTING
//!   member consents to it (`add`, which prints the `AddKey` ticket signed at
//!   the new key's current generation), and the new device submits that ticket
//!   under ITS OWN signature (`join`) — the frame origin is the key being
//!   admitted, so no possession proof rides the payload.
//! - `set-name`, `set-profile` — display text on the origin's account.
//!
//! Program output stays `println!` (a CLI's stdout is not logging); the
//! password crosses on stdin, never a flag.

use std::io::BufRead;
use std::path::PathBuf;

use commonware_cryptography::{Signer as _, ed25519};
use identity::{AccountView, IdentityMsg, IdentityQuery, IdentityReply, KeyScheme};

use crate::cli_args::NodeAddr;
use crate::config::{self, hex_bytes};
use crate::cred_cli::VerbCtx;
use crate::node_http;
use crate::userkey_cli::{load_user_signer, user_frame};

type AccountResult = Result<(), Box<dyn std::error::Error>>;

/// `ducktape account <verb>`. `--node`/`-n` are the shared [`NodeAddr`] group,
/// `global` so they attach in any position.
#[derive(Debug, clap::Args)]
pub(crate) struct AccountArgs {
    #[command(subcommand)]
    cmd: AccountCmd,
    #[command(flatten)]
    addr: NodeAddr,
    /// path to the user key file (defaults to the keystore's active wallet)
    #[arg(long, value_name = "PATH", global = true)]
    key: Option<PathBuf>,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum AccountCmd {
    /// found an account for this user key (a user-signed `create`)
    Create {
        /// the account's display name (not unique — the number is the id)
        #[arg(long, value_name = "NAME")]
        name: String,
    },
    /// print one account: this key's by default, else by number or member key
    Show {
        /// the account number
        #[arg(long, value_name = "N", conflicts_with = "pubkey")]
        number: Option<u64>,
        /// a member key (hex) of the account to show
        #[arg(long, value_name = "HEX")]
        pubkey: Option<String>,
    },
    /// the keys associated with this account
    #[command(subcommand)]
    Key(KeyCmd),
    /// rename this key's account
    SetName {
        #[arg(long, value_name = "NAME")]
        name: String,
    },
    /// set this key's account avatar reference and/or bio
    SetProfile {
        /// a duckfs path (`/shared/attachments/avatars/<sha16>.<ext>`)
        #[arg(long, value_name = "PATH")]
        avatar: Option<String>,
        /// a short status line
        #[arg(long, value_name = "TEXT")]
        bio: Option<String>,
    },
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum KeyCmd {
    /// list this key's account keys
    List,
    /// print THIS device's public key, for an existing member to `key add`
    Approve,
    /// (an existing member) consent to admitting a key: prints the ticket for `key join`
    Add {
        /// the key (hex) being admitted
        #[arg(long, value_name = "HEX")]
        pubkey: String,
        /// the key's scheme
        #[arg(long, value_enum, default_value_t = SchemeArg::Ed25519)]
        scheme: SchemeArg,
        /// a human label for the key (e.g. "phone")
        #[arg(long, value_name = "TEXT")]
        label: Option<String>,
    },
    /// (the new device) submit an add-key ticket, signed by ITS key
    Join {
        /// the ticket `key add` printed
        #[arg(long, value_name = "JSON")]
        ticket: String,
    },
    /// drop a key from this account (never the last one)
    Remove {
        /// the key (hex) to drop
        #[arg(long, value_name = "HEX")]
        pubkey: String,
    },
}

/// `--scheme` as clap sees it; the wire type is [`KeyScheme`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum SchemeArg {
    Ed25519,
    Secp256k1,
    Secp256r1,
}

impl From<SchemeArg> for KeyScheme {
    fn from(arg: SchemeArg) -> Self {
        match arg {
            SchemeArg::Ed25519 => KeyScheme::Ed25519,
            SchemeArg::Secp256k1 => KeyScheme::Secp256k1,
            SchemeArg::Secp256r1 => KeyScheme::Secp256r1,
        }
    }
}

/// the scheme's wire token, for prose output.
fn scheme_token(scheme: KeyScheme) -> &'static str {
    match scheme {
        KeyScheme::Ed25519 => "ed25519",
        KeyScheme::Secp256k1 => "secp256k1",
        KeyScheme::Secp256r1 => "secp256r1",
    }
}

/// Dispatch one `account` verb. ONE visible dispatch, nothing in the arms but
/// delegation; the password is read off stdin by the verbs that sign.
pub(crate) fn run(args: AccountArgs) -> AccountResult {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let AccountArgs { cmd, addr, key } = args;
    let ctx = VerbCtx { addr, key };
    match cmd {
        AccountCmd::Create { name } => cmd_create(&ctx, name, &mut stdin),
        AccountCmd::Show { number, pubkey } => cmd_show(&ctx, number, pubkey),
        AccountCmd::Key(KeyCmd::List) => cmd_key_list(&ctx),
        AccountCmd::Key(KeyCmd::Approve) => cmd_key_approve(&ctx),
        AccountCmd::Key(KeyCmd::Add {
            pubkey,
            scheme,
            label,
        }) => cmd_key_add(&ctx, pubkey, scheme.into(), label, &mut stdin),
        AccountCmd::Key(KeyCmd::Join { ticket }) => cmd_key_join(&ctx, ticket, &mut stdin),
        AccountCmd::Key(KeyCmd::Remove { pubkey }) => cmd_key_remove(&ctx, pubkey, &mut stdin),
        AccountCmd::SetName { name } => cmd_set_name(&ctx, name, &mut stdin),
        AccountCmd::SetProfile { avatar, bio } => cmd_set_profile(&ctx, avatar, bio, &mut stdin),
    }
}

// ============================================================================
// verbs
// ============================================================================

fn cmd_create(ctx: &VerbCtx, name: String, stdin: &mut impl BufRead) -> AccountResult {
    let base = ctx.http_base()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let height = submit_identity(&base, &user, &create_msg(name))?;
    let account = account_of_key(&base, user.public_key().as_ref())?.ok_or_else(|| {
        format!("committed at height {height}, but the key resolves to no account")
    })?;
    println!("account {} created at height {height}", account.number);
    print_account(&account);
    Ok(())
}

fn cmd_show(ctx: &VerbCtx, number: Option<u64>, pubkey: Option<String>) -> AccountResult {
    let base = ctx.http_base()?;
    let query = match (number, pubkey) {
        (Some(number), _) => IdentityQuery::Get { number },
        (None, Some(hex)) => IdentityQuery::OfKey {
            key: config::unhex(&hex).map_err(|e| format!("--pubkey hex: {e}"))?,
        },
        (None, None) => IdentityQuery::OfKey {
            key: active_pubkey(ctx)?,
        },
    };
    let account =
        account_reply(query_identity(&base, &query)?)?.ok_or("no such account on this node")?;
    print_account(&account);
    Ok(())
}

fn cmd_key_list(ctx: &VerbCtx) -> AccountResult {
    let base = ctx.http_base()?;
    let account = own_account(&base, &active_pubkey(ctx)?)?;
    print_keys(&account);
    Ok(())
}

/// `key approve` — no node, no password: the encrypted key file carries its
/// pubkey in the clear, and that is all an existing member needs.
fn cmd_key_approve(ctx: &VerbCtx) -> AccountResult {
    let pubkey = hex_bytes(&active_pubkey(ctx)?);
    println!("ed25519 {pubkey}");
    eprintln!(
        "an existing member admits it with:\n    ducktape account key add --pubkey {pubkey} --scheme ed25519"
    );
    Ok(())
}

fn cmd_key_add(
    ctx: &VerbCtx,
    pubkey: String,
    scheme: KeyScheme,
    label: Option<String>,
    stdin: &mut impl BufRead,
) -> AccountResult {
    let new_key = config::unhex(&pubkey).map_err(|e| format!("--pubkey hex: {e}"))?;
    let wellformed = scheme.pubkey_wellformed(&new_key);
    if !wellformed {
        return Err(format!("--pubkey is not a well-formed {} key", scheme_token(scheme)).into());
    }
    let base = ctx.http_base()?;
    let chain_id = ctx.workspace()?.service.chain_id;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    // the consent signs the new key's CURRENT generation, so it is single-use:
    // the module advances the generation on admission.
    let generation = gen_reply(query_identity(
        &base,
        &IdentityQuery::KeyGen {
            key: new_key.clone(),
        },
    )?)?;
    let ticket = add_key_ticket(&user, &chain_id, scheme, &new_key, label, generation);
    println!("{ticket}");
    eprintln!("on the new device, run:\n    ducktape account key join --ticket '{ticket}'");
    Ok(())
}

fn cmd_key_join(ctx: &VerbCtx, ticket: String, stdin: &mut impl BufRead) -> AccountResult {
    let base = ctx.http_base()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let height = node_http::submit_frame(&base, &join_frame(&user, &ticket)?)?;
    let account = own_account(&base, user.public_key().as_ref())?;
    println!("joined account {} at height {height}", account.number);
    print_keys(&account);
    Ok(())
}

fn cmd_key_remove(ctx: &VerbCtx, pubkey: String, stdin: &mut impl BufRead) -> AccountResult {
    let key = config::unhex(&pubkey).map_err(|e| format!("--pubkey hex: {e}"))?;
    let base = ctx.http_base()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let height = submit_identity(&base, &user, &IdentityMsg::RemoveKey { key })?;
    println!("removed at height {height}");
    Ok(())
}

fn cmd_set_name(ctx: &VerbCtx, name: String, stdin: &mut impl BufRead) -> AccountResult {
    let base = ctx.http_base()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let height = submit_identity(&base, &user, &IdentityMsg::SetName { name })?;
    println!("renamed at height {height}");
    Ok(())
}

fn cmd_set_profile(
    ctx: &VerbCtx,
    avatar: Option<String>,
    bio: Option<String>,
    stdin: &mut impl BufRead,
) -> AccountResult {
    let base = ctx.http_base()?;
    let user = load_user_signer(&ctx.key_path()?, stdin)?;
    let height = submit_identity(&base, &user, &IdentityMsg::SetProfile { avatar, bio })?;
    println!("profile set at height {height}");
    Ok(())
}

// ============================================================================
// pure builders (unit-tested)
// ============================================================================

/// the founding op: the CLI's key is always ed25519, and the frame signature
/// is its possession proof.
fn create_msg(name: String) -> IdentityMsg {
    IdentityMsg::Create {
        name,
        scheme: KeyScheme::Ed25519,
    }
}

/// the `AddKey` ticket an existing member mints for `new_key` (of `scheme`) at
/// its current `generation` on `chain_id`: ONE json line, exactly the payload
/// the new device submits.
fn add_key_ticket(
    user: &ed25519::PrivateKey,
    chain_id: &str,
    scheme: KeyScheme,
    new_key: &[u8],
    label: Option<String>,
    generation: u64,
) -> String {
    let msg = IdentityMsg::AddKey {
        scheme,
        label,
        authorizer: config::ed25519_authorizer(user, chain_id, scheme, new_key, generation),
    };
    String::from_utf8(identity::encode_msg(&msg)).expect("json is utf-8")
}

/// the joining device's frame: the ticket bytes VERBATIM (the member's proof
/// is over them), signed by the key being admitted. A ticket that is not an
/// `AddKey` is refused here, before any unlock reaches the node.
fn join_frame(user: &ed25519::PrivateKey, ticket: &str) -> Result<Vec<u8>, String> {
    let is_add_key = matches!(
        identity::decode_msg(ticket.as_bytes())?,
        IdentityMsg::AddKey { .. }
    );
    if !is_add_key {
        return Err("--ticket is not an add-key ticket (from `ducktape account key add`)".into());
    }
    Ok(user_frame(user, "identity", ticket.as_bytes().to_vec()))
}

// ============================================================================
// node round-trips
// ============================================================================

/// sign `msg` with `user` and submit it as one frame; the commit height.
fn submit_identity(
    base: &str,
    user: &ed25519::PrivateKey,
    msg: &IdentityMsg,
) -> Result<u64, Box<dyn std::error::Error>> {
    node_http::submit_frame(
        base,
        &user_frame(user, "identity", identity::encode_msg(msg)),
    )
}

fn query_identity(
    base: &str,
    query: &IdentityQuery,
) -> Result<IdentityReply, Box<dyn std::error::Error>> {
    let value = node_http::query(base, "identity", serde_json::to_value(query)?)?;
    Ok(serde_json::from_value(value)?)
}

fn account_reply(reply: IdentityReply) -> Result<Option<AccountView>, Box<dyn std::error::Error>> {
    match reply {
        IdentityReply::Account(account) => Ok(account),
        IdentityReply::Accounts(_) | IdentityReply::Gen(_) => {
            Err(format!("unexpected identity reply: {reply:?}").into())
        }
    }
}

fn accounts_reply(reply: IdentityReply) -> Result<Vec<AccountView>, Box<dyn std::error::Error>> {
    match reply {
        IdentityReply::Accounts(accounts) => Ok(accounts),
        IdentityReply::Account(_) | IdentityReply::Gen(_) => {
            Err(format!("unexpected identity reply: {reply:?}").into())
        }
    }
}

fn gen_reply(reply: IdentityReply) -> Result<u64, Box<dyn std::error::Error>> {
    match reply {
        IdentityReply::Gen(generation) => Ok(generation),
        IdentityReply::Account(_) | IdentityReply::Accounts(_) => {
            Err(format!("unexpected identity reply: {reply:?}").into())
        }
    }
}

/// the account `key` belongs to, if any — THE resolver every consumer reads.
pub(crate) fn account_of_key(
    base: &str,
    key: &[u8],
) -> Result<Option<AccountView>, Box<dyn std::error::Error>> {
    account_reply(query_identity(
        base,
        &IdentityQuery::OfKey { key: key.to_vec() },
    )?)
}

/// [`account_of_key`] for the local user key, loud when it is on no account.
pub(crate) fn own_account(
    base: &str,
    key: &[u8],
) -> Result<AccountView, Box<dyn std::error::Error>> {
    account_of_key(base, key)?
        .ok_or("this user key belongs to no account — `ducktape account create` first".into())
}

/// every account, paged through `All` by number.
pub(crate) fn all_accounts(base: &str) -> Result<Vec<AccountView>, Box<dyn std::error::Error>> {
    let mut all = Vec::new();
    let mut from = 1;
    loop {
        let page = accounts_reply(query_identity(
            base,
            &IdentityQuery::All {
                from,
                limit: identity::MAX_QUERY_LIMIT,
            },
        )?)?;
        let short_page = (page.len() as u64) < identity::MAX_QUERY_LIMIT;
        let Some(last) = page.last() else {
            return Ok(all);
        };
        from = last.number + 1;
        all.extend(page);
        if short_page {
            return Ok(all);
        }
    }
}

/// Resolve an account named on the command line: a decimal number is used as
/// is (never dialing), anything else is a display name matched over every
/// account — ambiguity and absence are loud errors.
pub(crate) fn resolve_account(base: &str, input: &str) -> Result<u64, Box<dyn std::error::Error>> {
    if let Ok(number) = input.parse::<u64>() {
        gateway::validate_account_number(number)?;
        return Ok(number);
    }
    let accounts = all_accounts(base)?;
    let matches: Vec<&AccountView> = accounts.iter().filter(|a| a.name == input).collect();
    match matches.as_slice() {
        [only] => Ok(only.number),
        [] => Err(format!("no account named {input:?} (nor an account number)").into()),
        many => {
            let numbers: Vec<String> = many.iter().map(|a| a.number.to_string()).collect();
            Err(format!(
                "account name {input:?} is ambiguous across {} accounts — pass a number: {}",
                many.len(),
                numbers.join(", ")
            )
            .into())
        }
    }
}

/// the active key's pubkey WITHOUT unlocking it: the encrypted file carries it
/// in the clear, so the read verbs never ask for a password.
fn active_pubkey(ctx: &VerbCtx) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(keystore::userkey::read_user_key_file(&ctx.key_path()?)?.pubkey)
}

fn print_account(account: &AccountView) {
    println!("number={} name={}", account.number, account.name);
    if let Some(avatar) = &account.avatar {
        println!("avatar={avatar}");
    }
    if let Some(bio) = &account.bio {
        println!("bio={bio}");
    }
    print_keys(account);
}

fn print_keys(account: &AccountView) {
    for key in &account.keys {
        let label = key.label.as_deref().unwrap_or("");
        println!(
            "key={} {} {label}",
            scheme_token(key.scheme),
            hex_bytes(&key.pubkey)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use commonware_codec::DecodeExt as _;

    /// a Parser wrapper so tests can exercise the derived verb SHAPE the same
    /// way `main.rs`'s integrator will.
    #[derive(clap::Parser)]
    struct TestCli {
        #[command(subcommand)]
        #[allow(dead_code)]
        cmd: AccountCmd,
    }

    fn signer(seed: u8) -> ed25519::PrivateKey {
        ed25519::PrivateKey::decode([seed; 32].as_slice()).unwrap()
    }

    #[test]
    fn the_verb_tree_is_create_show_key_set_name_set_profile() {
        use clap::CommandFactory as _;
        let cmd = TestCli::command();
        for name in ["create", "show", "key", "set-name", "set-profile"] {
            assert!(cmd.find_subcommand(name).is_some(), "verb {name} missing");
        }
        let key = cmd.find_subcommand("key").unwrap();
        for name in ["list", "approve", "add", "join", "remove"] {
            assert!(
                key.find_subcommand(name).is_some(),
                "key verb {name} missing"
            );
        }
    }

    #[test]
    fn create_decodes_to_create_for_an_ed25519_origin() {
        let msg = create_msg("alice".into());
        assert_eq!(
            identity::decode_msg(&identity::encode_msg(&msg)).unwrap(),
            IdentityMsg::Create {
                name: "alice".into(),
                scheme: KeyScheme::Ed25519,
            }
        );
    }

    #[test]
    fn key_add_ticket_decodes_to_add_key_and_its_consent_verifies() {
        let member = signer(5);
        let new_key = signer(6).public_key().as_ref().to_vec();
        let ticket = add_key_ticket(
            &member,
            "chain-a",
            KeyScheme::Ed25519,
            &new_key,
            Some("phone".into()),
            2,
        );
        assert_eq!(ticket.lines().count(), 1, "one json line, pasteable");
        let IdentityMsg::AddKey {
            scheme,
            label,
            authorizer,
        } = identity::decode_msg(ticket.as_bytes()).unwrap()
        else {
            panic!("a ticket is an AddKey");
        };
        assert_eq!(scheme, KeyScheme::Ed25519);
        assert_eq!(label.as_deref(), Some("phone"));
        assert_eq!(authorizer.key, member.public_key().as_ref());
        // the module's own check, verbatim: at the generation the ticket was
        // minted for — and at no other.
        let preimage = |generation| {
            identity::add_key_preimage("chain-a", KeyScheme::Ed25519, &new_key, generation)
        };
        assert!(KeyScheme::Ed25519.verify(
            &authorizer.key,
            identity::IDENTITY_ADD_KEY_NS,
            &preimage(2),
            &authorizer.proof,
        ));
        assert!(!KeyScheme::Ed25519.verify(
            &authorizer.key,
            identity::IDENTITY_ADD_KEY_NS,
            &preimage(3),
            &authorizer.proof,
        ));
    }

    #[test]
    fn key_join_wraps_the_ticket_verbatim_under_the_joining_key() {
        let member = signer(5);
        let joiner = signer(6);
        let ticket = add_key_ticket(
            &member,
            "chain-a",
            KeyScheme::Ed25519,
            joiner.public_key().as_ref(),
            None,
            0,
        );
        let frame = join_frame(&joiner, &ticket).unwrap();
        let (origin, msg) = node::decode_frame(&frame).expect("frame verifies");
        assert_eq!(
            origin,
            sdk::Origin::External(joiner.public_key().as_ref().to_vec()),
            "the origin is the key being admitted"
        );
        assert_eq!(msg.target, "identity");
        assert_eq!(
            msg.payload,
            ticket.as_bytes(),
            "the member's proof is over these exact bytes"
        );

        // anything but an AddKey is refused before a node is dialed.
        let not_a_ticket =
            String::from_utf8(identity::encode_msg(&create_msg("x".into()))).unwrap();
        assert!(join_frame(&joiner, &not_a_ticket).is_err());
        assert!(join_frame(&joiner, "not json").is_err());
    }

    /// a decimal never dials: the base here is a dead url on purpose.
    #[test]
    fn resolve_account_takes_a_number_offline_and_refuses_zero() {
        assert_eq!(resolve_account("http://127.0.0.1:9", "12").unwrap(), 12);
        assert!(resolve_account("http://127.0.0.1:9", "0").is_err());
    }
}
