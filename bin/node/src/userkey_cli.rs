//! User identity lifecycle and signing command surface.
//!
//! These commands are synchronous operator tooling. Keeping them outside the
//! live node entrypoint makes their stdin-only secret handling and output
//! contracts independently testable without mixing them into runtime boot.

use std::path::PathBuf;

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519};

use crate::{config, userkey};
use config::hex_bytes;

type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// the typed `ducktape user` grammar — clap derive. this is only the SHAPE
/// (verbs, flags, help); the handlers below own the work. every secret
/// (password, mnemonic) still crosses the process boundary via STDIN ONLY,
/// one newline-delimited field per line — NEVER a flag, which would leak into
/// shell history / `ps`. clap enforces the required flags; numeric flags are
/// typed so clap validates them at parse.
// each variant carries its verb's typed args; parsed once and consumed.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, clap::Subcommand)]
pub(crate) enum UserCmd {
    /// encrypted user key lifecycle
    Key(UserKeyArgs),
    /// mint a bind certificate binding a node to this user identity
    SignBind(NodeBindArgs),
    /// mint an unbind certificate evicting a node from this user identity
    SignUnbind(NodeBindArgs),
    /// print a new ed25519 device's possession proof over the add-member preimage
    SignPossession(PossessionArgs),
    /// consent (as an existing member) to admitting a new account key
    SignAddMember(AddMemberArgs),
    /// evict a member key from the account
    SignRemoveMember(RemoveMemberArgs),
    /// sign one canonical gateway route statement
    SignGatewayRoute(GatewayRouteArgs),
    /// wrap a module op payload in a user-signed submit frame (hex)
    SignFrame(FrameArgs),
    /// sign one owner control-plane request (the `/v1/admin` per-request PoP)
    SignAdmin(AdminArgs),
    /// redeem a CLIENT invite as this user key over a member node
    RedeemInvite(RedeemArgs),
    /// print the base64url WebAuthn challenge a passkey must sign to join
    WebauthnChallenge(EnrollArgs),
    /// print the hex bytes a software P256 key must ECDSA-sign to join
    P256Payload(EnrollArgs),
}

/// `user key` lifecycle subcommands.
#[derive(Debug, clap::Args)]
pub(crate) struct UserKeyArgs {
    #[command(subcommand)]
    sub: UserKeyCmd,
}

#[derive(Debug, clap::Subcommand)]
pub(crate) enum UserKeyCmd {
    /// generate a fresh seed, write encrypted v1, print mnemonic then pubkey
    Init(KeyOutArgs),
    /// restore from a mnemonic line, write encrypted v1, print pubkey
    Restore(KeyOutArgs),
    /// verify a key file's password; print its pubkey
    Unlock(KeyPathArgs),
    /// print the 24-word mnemonic for a key file
    Reveal(KeyPathArgs),
    /// report `absent` or `encrypted <pubkey>`
    Status(KeyPathArgs),
}

/// verbs that WRITE a new key file: the destination path (default `user.key`).
#[derive(Debug, clap::Args)]
pub(crate) struct KeyOutArgs {
    /// write the key file here
    #[arg(long, value_name = "PATH", default_value = "user.key")]
    out: PathBuf,
}

/// verbs that READ an existing key file.
#[derive(Debug, clap::Args)]
pub(crate) struct KeyPathArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
}

/// bind/unbind a node to this user identity — identical flag shape for both.
#[derive(Debug, clap::Args)]
pub(crate) struct NodeBindArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the network's chain id
    #[arg(long = "chain-id", value_name = "ID")]
    chain_id: String,
    /// the hex node pubkey to bind/unbind
    #[arg(long = "node-pub", value_name = "HEX")]
    node_pub: String,
    /// monotonic per-account nonce
    #[arg(long, value_name = "N")]
    nonce: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct PossessionArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the network's chain id
    #[arg(long = "chain-id", value_name = "ID")]
    chain_id: String,
    /// the account id (hex) this device is joining
    #[arg(long = "account-id", value_name = "HEX")]
    account_id: String,
    /// monotonic per-account nonce
    #[arg(long, value_name = "N")]
    nonce: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct AddMemberArgs {
    /// path to the LOCAL member's user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the network's chain id
    #[arg(long = "chain-id", value_name = "ID")]
    chain_id: String,
    /// the account id (hex) the new key joins
    #[arg(long = "account-id", value_name = "HEX")]
    account_id: String,
    /// the new key (hex) being admitted
    #[arg(long = "new-key", value_name = "HEX")]
    new_key: String,
    /// the new key's kind: ed25519 | p256 | webauthn_p256
    #[arg(long = "new-kind", value_name = "KIND")]
    new_kind: String,
    /// monotonic per-account nonce
    #[arg(long, value_name = "N")]
    nonce: u64,
    /// optional human label for the new key
    #[arg(long, value_name = "S")]
    label: Option<String>,
    /// the new key's possession proof (`MemberProof` json)
    #[arg(long, value_name = "JSON")]
    possession: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct RemoveMemberArgs {
    /// path to the LOCAL member's user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the network's chain id
    #[arg(long = "chain-id", value_name = "ID")]
    chain_id: String,
    /// the account id (hex) to evict from
    #[arg(long = "account-id", value_name = "HEX")]
    account_id: String,
    /// the member key (hex) to evict
    #[arg(long = "target-key", value_name = "HEX")]
    target_key: String,
    /// monotonic per-account nonce
    #[arg(long, value_name = "N")]
    nonce: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct GatewayRouteArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the canonical route statement (json), byte-capped
    #[arg(long, value_name = "JSON")]
    statement: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct FrameArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the destination module
    #[arg(long, value_name = "MODULE")]
    target: String,
    /// the frame's ordering/dedup tie-breaker (any u64)
    #[arg(long, value_name = "N")]
    seq: u64,
}

#[derive(Debug, clap::Args)]
pub(crate) struct AdminArgs {
    /// path to the user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
    /// the HTTP method of the admin request
    #[arg(long, value_name = "M")]
    method: String,
    /// the request path and query
    #[arg(long, value_name = "PATH-AND-QUERY")]
    path: String,
    /// the target node's consensus key (hex)
    #[arg(long = "node-key", value_name = "HEX")]
    node_key: String,
}

#[derive(Debug, clap::Args)]
pub(crate) struct RedeemArgs {
    /// the one-line invite blob to redeem
    #[arg(value_name = "INVITE-BLOB")]
    blob: String,
    /// the member node's http base (e.g. `http://host:port`)
    #[arg(long, value_name = "URL")]
    node: Option<String>,
    /// resolve the node through a registered workspace's chain id
    #[arg(short = 'n', long = "network", value_name = "CHAIN-ID")]
    network: Option<String>,
    /// path to the encrypted user key file
    #[arg(long, value_name = "PATH")]
    key: PathBuf,
}

/// the pure enrollment-preimage verbs (no key, no signing): they only compute
/// the bytes a joining key must sign, so they share one flag shape.
#[derive(Debug, clap::Args)]
pub(crate) struct EnrollArgs {
    /// the network's chain id
    #[arg(long = "chain-id", value_name = "ID")]
    chain_id: String,
    /// the account id (hex) the new key joins
    #[arg(long = "account-id", value_name = "HEX")]
    account_id: String,
    /// the new key (hex) being enrolled
    #[arg(long = "new-key", value_name = "HEX")]
    new_key: String,
    /// monotonic per-account nonce
    #[arg(long, value_name = "N")]
    nonce: u64,
}

/// Run one verb of the `ducktape user` family. secrets cross via stdin only
/// (see the module header), so `run` opens stdin once and every handler reads
/// its secret fields from it — never from `cmd`.
pub(super) fn run(cmd: UserCmd) -> CommandResult {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    match cmd {
        UserCmd::Key(args) => cmd_user_key(args, &mut stdin),
        UserCmd::SignBind(args) => cmd_user_sign_bind(args, &mut stdin),
        UserCmd::SignUnbind(args) => cmd_user_sign_unbind(args, &mut stdin),
        UserCmd::SignPossession(args) => cmd_user_sign_possession(args, &mut stdin),
        UserCmd::SignAddMember(args) => cmd_user_sign_add_member(args, &mut stdin),
        UserCmd::SignRemoveMember(args) => cmd_user_sign_remove_member(args, &mut stdin),
        UserCmd::SignGatewayRoute(args) => cmd_user_sign_gateway_route(args, &mut stdin),
        UserCmd::SignFrame(args) => cmd_user_sign_frame(args, &mut stdin),
        UserCmd::SignAdmin(args) => cmd_user_sign_admin(args, &mut stdin),
        UserCmd::RedeemInvite(args) => cmd_user_redeem_invite(args, &mut stdin),
        UserCmd::WebauthnChallenge(args) => cmd_user_webauthn_challenge(args),
        UserCmd::P256Payload(args) => cmd_user_p256_payload(args),
    }
}

// ============================================================================
// user-key lifecycle verbs (init/restore/unlock/reveal/encrypt/status) — see
// docs/superpowers/specs/2026-07-07-identity-onboarding-design.md's "CLI
// verbs" section for the binding stdin/stdout contract. every secret
// (password, mnemonic) crosses the process boundary via STDIN ONLY, one
// newline-delimited field per line in the documented order — never argv/env,
// which would leak into shell history / `ps`. each verb below is split into
// a `user_key_*` core (takes the parsed stdin, returns the value to print —
// directly unit-testable without capturing stdout) and a thin `cmd_user_key_*`
// wrapper that prints it; the wrapper is what `run()`'s dispatch calls.
// ============================================================================

/// read one line from `stdin`, minus its trailing newline — the stdin-only
/// convention every secret field crosses the process boundary through.
/// errors only on true EOF (nothing at all, not even a newline): a caller
/// that doesn't pipe the expected field. an explicit empty line (just `\n`)
/// is NOT an error here — callers that need a non-empty value (passwords)
/// reject that on their own terms (`check_password_len`), with a clearer
/// message than a generic "missing" would give.
fn read_stdin_line(stdin: &mut impl std::io::BufRead, field: &str) -> Result<String, String> {
    let mut line = String::new();
    let n = stdin
        .read_line(&mut line)
        .map_err(|e| format!("read {field} from stdin: {e}"))?;
    if n == 0 {
        return Err(format!("missing {field} on stdin"));
    }
    while line.ends_with('\n') || line.ends_with('\r') {
        line.pop();
    }
    Ok(line)
}

/// True when this process's stdin is an interactive terminal. Only then do we
/// prompt / mask — a pipe stays byte-for-byte the old plain-stdin contract.
fn stdin_is_tty() -> bool {
    // SAFETY: isatty only queries a descriptor's mode; no memory is touched.
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

/// A field name that implies key material — echo is masked when it's typed at a
/// terminal. `payload-hex` is NOT a secret (it's a public op body) and is not
/// masked.
fn field_is_secret(field: &str) -> bool {
    field.contains("password") || field.contains("mnemonic")
}

/// RAII guard that disables terminal echo on stdin and restores it on Drop —
/// including panic unwind and early-return error paths. `engage` returns `None`
/// when stdin has no termios (restore is then a no-op), so the caller reads
/// unmasked rather than failing.
struct EchoOff {
    saved: libc::termios,
}

impl EchoOff {
    fn engage() -> Option<Self> {
        let fd = libc::STDIN_FILENO;
        // SAFETY: termios is a plain C struct of integers/arrays; tcgetattr
        // fills the zeroed value before we read it.
        let mut term: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(fd, &mut term) } != 0 {
            return None;
        }
        let saved = term;
        term.c_lflag &= !libc::ECHO;
        // SAFETY: term is a valid termios we just read back and edited.
        if unsafe { libc::tcsetattr(fd, libc::TCSANOW, &term) } != 0 {
            return None;
        }
        Some(Self { saved })
    }
}

impl Drop for EchoOff {
    fn drop(&mut self) {
        // SAFETY: `saved` is the termios we captured; best-effort restore.
        unsafe { libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &self.saved) };
    }
}

/// Run `read` to consume one stdin field. When `is_tty`, first prompt
/// `<field>: ` to stderr and — for secret fields — mask echo for the read
/// (restoring on the way out and emitting the newline the mask swallowed).
/// When not a tty this is exactly `read()`: no prompt, no termios, so piped
/// stdin is unchanged. `is_tty` is a parameter so the non-tty path is unit
/// testable without a controlling terminal.
fn with_prompt<T>(field: &str, is_tty: bool, read: impl FnOnce() -> T) -> T {
    if !is_tty {
        return read();
    }
    eprint!("{field}: ");
    let _ = std::io::Write::flush(&mut std::io::stderr());
    let secret = field_is_secret(field);
    let _echo_off = if secret { EchoOff::engage() } else { None };
    let out = read();
    if secret {
        // masked echo swallowed the user's Enter — supply the missing newline.
        eprintln!();
    }
    out
}

/// [`read_stdin_line`] fronted by the tty prompt/mask wrapper — the entry point
/// every secret-bearing verb reads its fields through.
fn prompt_stdin_line(stdin: &mut impl std::io::BufRead, field: &str) -> Result<String, String> {
    with_prompt(field, stdin_is_tty(), || read_stdin_line(stdin, field))
}

/// the password floor for newly encrypted keys,
/// enforced before any file is touched. counts scalar chars, not bytes, so a
/// multi-byte-but-short password isn't laundered past the floor.
const MIN_PASSWORD_LEN: usize = 8;

fn check_password_len(password: &str) -> Result<(), String> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_LEN} characters"
        ));
    }
    Ok(())
}

/// `path`'s raw trimmed encrypted line.
fn read_key_line(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let line = text.trim();
    if line.is_empty() {
        return Err(format!("{path:?} is empty"));
    }
    Ok(line.to_string())
}

/// Resolve the user signer from an encrypted key. Password is stdin's first
/// line for every signing verb.
fn load_user_signer(
    key_path: &std::path::Path,
    stdin: &mut impl std::io::BufRead,
) -> Result<ed25519::PrivateKey, Box<dyn std::error::Error>> {
        let password = prompt_stdin_line(stdin, "password")?;
    let line = read_key_line(key_path)?;
    Ok(userkey::open_user_key(&line, &password)?)
}

/// `user-key init` core — see [`cmd_user_key_init`] for the print contract.
/// returns `(mnemonic, pubkey-hex)` so tests can assert both independently.
fn user_key_init(
    args: KeyOutArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let password = prompt_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
    let words = userkey::mnemonic_of_seed(&seed);
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&args.out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice()).expect("32 random bytes decode");
    Ok((words, hex_bytes(key.public_key().as_ref())))
}

/// `user-key init --out <path>` — stdin: password. Generates a fresh seed,
/// writes the encrypted shape (refuses to overwrite via `create_new`), and prints the 24-word
/// mnemonic line THEN the pubkey-hex line — pubkey is the LAST stdout line
/// (the `run_verb`/`last_line` contract), mnemonic is the line before it.
fn cmd_user_key_init(args: KeyOutArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    let (words, pubkey_hex) = user_key_init(args, stdin)?;
    println!("{words}");
    println!("{pubkey_hex}");
    Ok(())
}

/// `user-key restore` core — see [`cmd_user_key_restore`].
fn user_key_restore(
    args: KeyOutArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let mnemonic = prompt_stdin_line(stdin, "mnemonic")?;
    let password = prompt_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let seed = userkey::seed_of_mnemonic(&mnemonic)?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&args.out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("restored seed is not a valid ed25519 secret: {e}"))?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key restore --out <path>` — stdin: mnemonic line, then password
/// line. Validates the BIP39 checksum, writes the encrypted shape (refuses to overwrite),
/// prints the pubkey (the only stdout line).
fn cmd_user_key_restore(args: KeyOutArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_key_restore(args, stdin)?);
    Ok(())
}

/// `user-key unlock` core — see [`cmd_user_key_unlock`].
fn user_key_unlock(
    args: KeyPathArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let password = prompt_stdin_line(stdin, "password")?;
            let line = read_key_line(&args.key)?;
    let key = userkey::open_user_key(&line, &password)?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key unlock --key <path>` — stdin: password. Pure verification
/// (nothing persists); prints the pubkey on success, a clean error + nonzero
/// exit on a wrong password or a corrupt file.
fn cmd_user_key_unlock(args: KeyPathArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_key_unlock(args, stdin)?);
    Ok(())
}

/// `user-key reveal` core — see [`cmd_user_key_reveal`].
fn user_key_reveal(
    args: KeyPathArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let password = prompt_stdin_line(stdin, "password")?;
            let line = read_key_line(&args.key)?;
    let key = userkey::open_user_key(&line, &password)?;
    let seed_bytes = key.encode();
    let seed: [u8; 32] = seed_bytes
        .as_ref()
        .try_into()
        .map_err(|_| "decoded key is not a 32-byte seed".to_string())?;
    Ok(userkey::mnemonic_of_seed(&seed))
}

/// `user-key reveal --key <path>` — stdin: password. Prints the 24-word
/// mnemonic — the same encoding `init`/`restore` use, so it round-trips
/// through `user-key restore` to the identical pubkey.
fn cmd_user_key_reveal(args: KeyPathArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_key_reveal(args, stdin)?);
    Ok(())
}

/// `user-key status` core — see [`cmd_user_key_status`].
fn user_key_status(args: KeyPathArgs) -> Result<String, Box<dyn std::error::Error>> {
    if !args.key.exists() {
        return Ok("absent".to_string());
    }
    let enc = userkey::read_user_key_file(&args.key)?;
    Ok(format!("encrypted {}", hex_bytes(&enc.pubkey)))
}

/// `user-key status --key <path>` — no stdin. Prints `absent` or
/// `encrypted <pubkey-hex>` without touching a password.
fn cmd_user_key_status(args: KeyPathArgs) -> CommandResult {
    println!("{}", user_key_status(args)?);
    Ok(())
}

/// `user key [init|restore|unlock|reveal|status]`.
fn cmd_user_key(args: UserKeyArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    match args.sub {
        UserKeyCmd::Init(a) => cmd_user_key_init(a, stdin),
        UserKeyCmd::Restore(a) => cmd_user_key_restore(a, stdin),
        UserKeyCmd::Unlock(a) => cmd_user_key_unlock(a, stdin),
        UserKeyCmd::Reveal(a) => cmd_user_key_reveal(a, stdin),
        UserKeyCmd::Status(a) => cmd_user_key_status(a),
    }
}

/// `user-sign-bind` core — see [`cmd_user_sign_bind`].
fn user_sign_bind(
    args: NodeBindArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let node_pub = config::decode_key(&args.node_pub)?;

    let user = load_user_signer(&args.key, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_BIND_NS,
        &identity::bind_preimage(&args.chain_id, node_pub.as_ref(), args.nonce),
    );
    let msg = IdentityMsg::BindNode { authorizer };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-bind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>`
/// — mint a bind certificate binding `node-pub` to the user identity at
/// `--key` (decrypted with stdin's password line), at `chain-id`/`nonce`,
/// and print the ready-to-submit `IdentityMsg::BindNode` JSON as the last
/// (only) stdout line. `user_key` rides the payload — the node being bound is
/// the verified submit ORIGIN, never a payload field; the module resolves it
/// from the rpc transport, not from this CLI.
fn cmd_user_sign_bind(args: NodeBindArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_sign_bind(args, stdin)?);
    Ok(())
}

/// `user-sign-unbind` core — see [`cmd_user_sign_unbind`].
fn user_sign_unbind(
    args: NodeBindArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let node_pub = config::decode_key(&args.node_pub)?;

    let user = load_user_signer(&args.key, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_UNBIND_NS,
        &identity::unbind_preimage(&args.chain_id, node_pub.as_ref(), args.nonce),
    );
    let msg = IdentityMsg::UnbindNode {
        node_key: node_pub.as_ref().to_vec(),
        authorizer,
    };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-unbind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>`
/// — mint an unbind certificate evicting `node-pub` from the user identity at
/// `--key`, and print the ready-to-submit `IdentityMsg::UnbindNode` JSON as
/// the last stdout line. `node_key` (not `user_key`) rides the payload:
/// unbind carries no origin restriction — a surviving device evicts a lost
/// one by naming it directly, identified via the existing binding.
fn cmd_user_sign_unbind(args: NodeBindArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_sign_unbind(args, stdin)?);
    Ok(())
}

/// Sign one canonical gateway route and return a ready-to-submit `GatewayMsg`.
/// This remains a namespace-specific oracle: the CLI parses and validates the
/// complete bounded route before the user key signs it.
fn user_sign_gateway_route(
    args: GatewayRouteArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    if args.statement.len() > gateway::MAX_ROUTE_STATEMENT_JSON_BYTES {
        return Err("user-sign-gateway-route statement exceeds the byte cap".into());
    }
    let statement: gateway::RouteStatement = serde_json::from_str(&args.statement)?;
    let preimage = gateway::route_signing_preimage(&statement)?;
    let user = load_user_signer(&args.key, stdin)?;
    let message = gateway::GatewayMsg::SetRoute {
        statement,
        authorization: gateway::MemberAuthorization {
            signer: user.public_key().as_ref().to_vec(),
            signature: user
                .sign(gateway::GATEWAY_ROUTE_NS, &preimage)
                .as_ref()
                .to_vec(),
        },
    };
    Ok(String::from_utf8(gateway::encode_msg(&message)).expect("json is utf-8"))
}

fn cmd_user_sign_gateway_route(
    args: GatewayRouteArgs,
    stdin: &mut impl std::io::BufRead,
) -> CommandResult {
    println!("{}", user_sign_gateway_route(args, stdin)?);
    Ok(())
}

/// `user-sign-frame` core — see [`cmd_user_sign_frame`].
fn user_sign_frame(
    args: FrameArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    // stdin order: password first, then the payload as one hex line. The
    // payload is not a secret; it rides stdin because a 1 MiB chunk
    // frame would blow past OS argv limits.
    let user = load_user_signer(&args.key, stdin)?;
    let payload_hex = prompt_stdin_line(stdin, "payload-hex")?;
    let payload = config::unhex(&payload_hex).map_err(|e| format!("payload hex: {e}"))?;

    let frame = node::encode_frame(
        &user,
        args.seq,
        &sdk::Msg {
            target: args.target,
            payload,
        },
        None,
    );
    Ok(hex_bytes(&frame))
}

/// `user-sign-frame --key <path> --target <module> --seq <n>` — stdin:
/// password line, then one payload-hex line.
/// Wraps the payload in a `node` op frame signed by the user key and prints
/// the frame as hex (the only stdout line). POSTed raw to `/v1/submit/frame`,
/// the frame's verified signer becomes the op's `Origin::External` — the
/// authenticated authorship the frameless `/v1/submit`'s plaintext `origin`
/// convention cannot provide. `seq` is the frame's ordering/dedup tie-breaker
/// (same-payload resubmits need a fresh seq to survive the consensus lane's
/// content-digest replay guard); it is NOT tracked in state — any u64 works.
fn cmd_user_sign_frame(args: FrameArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_sign_frame(args, stdin)?);
    Ok(())
}

/// resolve the member node's HTTP base for `redeem-invite`: an explicit
/// `--node <url>` wins, else `-n/--network <id>` resolves through the registry
/// to the workspace node.toml's `http_listen`. a set-but-broken `--network`
/// (unknown/ambiguous workspace, or one with no http listen) is a loud error.
fn redeem_node(
    node: Option<&str>,
    network: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    if let Some(url) = node.filter(|url| !url.is_empty()) {
        return Ok(url.trim_end_matches('/').to_string());
    }
    if let Some(needle) = network.filter(|needle| !needle.is_empty()) {
        let (_dir, http) = config::resolve_network(needle)?;
        let base = http.ok_or_else(|| {
            format!(
                "network {needle:?} resolves to a workspace with no http listen \
                 (its node.toml sets no http_listen) — pass --node <http-url>"
            )
        })?;
        return Ok(base.trim_end_matches('/').to_string());
    }
    Err(
        "user-redeem-invite needs --node <http-base, e.g. http://host:port> or -n/--network <id>"
            .into(),
    )
}

/// `user-redeem-invite` core — see [`cmd_user_redeem_invite`].
///
/// redeems a CLIENT invite as this user key over a member node's frameless
/// `/v1/submit`: the token (not the submitter) authorizes the admission —
/// the serving node stamps its own identity as the frame origin, and
/// consensus verifies the token signature + this key's join proof inside the
/// op, granting submit-only client standing to the key on commit. the lane
/// already settles-then-answers (the receipt carries the committed height;
/// a deterministic reject comes back as the error body), so the printed
/// verdict IS the consensus outcome.
fn user_redeem_invite(
    args: RedeemArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let node = redeem_node(args.node.as_deref(), args.network.as_deref())?;

    // fail-closed expiry + envelope/token verification at decode.
    let invite = config::decode_invite(&args.blob)?;
    if invite.token.role != config::InviteRole::Client {
        return Err("this is a node (resident) invite — use `ducktape node join`".into());
    }
    // every invite is bearer (기명 dropped — see the join ADR): no target lock — this user
    // key redeems the client invite directly, bound by the join proof below and
    // made single-use by the nonce.
    let user = load_user_signer(&args.key, stdin)?;
    let binding = invite.descriptor.genesis_namespace();
    let proof = config::sign_join_proof(&user, binding.as_bytes(), &invite.token);

    let payload = serde_json::json!({ "redeem": {
        "issuer": invite.token.issuer.as_ref().to_vec(),
        "nonce": invite.token.nonce.to_vec(),
        "token_sig": invite.token.sig.encode().as_ref().to_vec(),
        "joiner": user.public_key().as_ref().to_vec(),
        "proof": proof.encode().as_ref().to_vec(),
        "role": invite.token.role.as_u8(),
        "expires_unix_secs": invite.token.expires_unix_secs,
    }});
    let resp = reqwest::blocking::Client::new()
        .post(format!("{node}/v1/submit"))
        .json(&serde_json::json!({ "target": "governance", "payload": payload }))
        .send()
        .map_err(|e| format!("POST {node}/v1/submit: {e}"))?;
    let status = resp.status();
    let body = resp.text().unwrap_or_default();
    if status.is_success() {
        let height = serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|v| v["height"].as_u64())
            .ok_or_else(|| format!("unexpected submit receipt: {body}"))?;
        return Ok(format!(
            "admitted: client standing committed at height {height}"
        ));
    }
    // a re-redeem by the SAME key is not an error worth failing a script over.
    if body.contains("already holds client standing") {
        return Ok("already admitted: this key holds client standing".into());
    }
    Err(format!("redemption rejected ({status}): {body}").into())
}

/// `user-redeem-invite <blob> (--node <http-base> | -n/--network <id>) --key <path>`
/// — stdin: password line. Redeems a CLIENT invite as this user key
/// and prints the consensus verdict; the key then submits via
/// `/v1/submit/frame` under its own signature.
fn cmd_user_redeem_invite(args: RedeemArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_redeem_invite(args, stdin)?);
    Ok(())
}

/// `user-sign-admin` core — see [`cmd_user_sign_admin`].
///
/// signs one owner control-plane request (ADR A5): the per-request PoP the
/// node's `/v1/admin/*` gate checks under `Public` exposure. the signed bytes
/// are `noded::admin::sign_admin`'s — the SAME function the verifier uses, so
/// the two can never drift. the freshness timestamp is minted here and returned
/// alongside the signature so the caller stamps the exact `ts` that was signed.
fn user_sign_admin(
    args: AdminArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    // the TARGET node's consensus key (hex): folded into the signed bytes so
    // this signature can never be replayed against another node.
    let node_key = config::unhex(&args.node_key).map_err(|e| format!("--node-key hex: {e}"))?;

    // stdin: password only — there is no payload.
    let user = load_user_signer(&args.key, stdin)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let sig = noded::admin::sign_admin(&user, &args.method, &args.path, &node_key, ts);
    let out = serde_json::json!({
        "key": hex_bytes(user.public_key().as_ref()),
        "ts": ts.to_string(),
        "sig": hex_bytes(sig.as_ref()),
    });
    Ok(out.to_string())
}

/// `user-sign-admin --key <path> --method <M> --path <path-and-query>
/// --node-key <hex>` — stdin: password line.
/// Prints one JSON line `{"key","ts","sig"}` the app turns into the
/// `x-ducktape-admin-*` headers of an owner control request. Fresh `ts` per
/// call (replay-bounded); node-key-bound (no cross-node replay).
fn cmd_user_sign_admin(args: AdminArgs, stdin: &mut impl std::io::BufRead) -> CommandResult {
    println!("{}", user_sign_admin(args, stdin)?);
    Ok(())
}

/// parse a `--new-kind` flag value into a [`identity::KeyKind`]. the CLI's own
/// key is always ed25519; `p256`/`webauthn_p256` name the kind of a DIFFERENT
/// key being admitted (whose possession proof comes from that key's holder --
/// a native signer, or the FIDO2 transport for a passkey).
fn parse_kind(s: &str) -> Result<identity::KeyKind, Box<dyn std::error::Error>> {
    match s {
        "ed25519" => Ok(identity::KeyKind::Ed25519),
        "p256" => Ok(identity::KeyKind::P256),
        "webauthn_p256" | "webauthn-p256" | "passkey" => Ok(identity::KeyKind::WebauthnP256),
        other => {
            Err(format!("unknown key kind {other:?} (want ed25519|p256|webauthn_p256)").into())
        }
    }
}

/// `user-sign-possession` core — see [`cmd_user_sign_possession`].
fn user_sign_possession(
    args: PossessionArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let account_id = config::unhex(&args.account_id)?;

    // this key proves it holds itself over the add-member preimage; its own
    // pubkey is `new_key`, and the node's user key is ed25519.
    let user = load_user_signer(&args.key, stdin)?;
    let new_key = user.public_key().as_ref().to_vec();
    let preimage = identity::add_member_preimage(
        &args.chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::Ed25519,
        args.nonce,
    );
    let proof = config::ed25519_possession(&user, identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    Ok(serde_json::to_string(&proof).expect("json is utf-8"))
}

/// `user-sign-possession --key <path> --chain-id <id> --account-id <hex> --nonce <n>`
/// — for a NEW ed25519 device joining an existing account: print the
/// possession-proof `MemberProof` JSON this device signs over the add-member
/// preimage (pair its `user-key status` pubkey with it). the existing member
/// then feeds both to `user-sign-add-member`.
fn cmd_user_sign_possession(
    args: PossessionArgs,
    stdin: &mut impl std::io::BufRead,
) -> CommandResult {
    println!("{}", user_sign_possession(args, stdin)?);
    Ok(())
}

/// `user-sign-add-member` core — see [`cmd_user_sign_add_member`].
fn user_sign_add_member(
    args: AddMemberArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let account_id = config::unhex(&args.account_id)?;
    let new_key = config::unhex(&args.new_key)?;
    let new_kind = parse_kind(&args.new_kind)?;
    let new_label = args.label;
    let possession: identity::MemberProof = serde_json::from_str(&args.possession)
        .map_err(|e| format!("--possession is not a MemberProof: {e}"))?;

    // the local user key is an existing member; it consents to admitting the
    // new key over the same preimage the new key proved possession of.
    let user = load_user_signer(&args.key, stdin)?;
    let preimage =
        identity::add_member_preimage(&args.chain_id, &account_id, &new_key, new_kind, args.nonce);
    let authorizer =
        config::ed25519_member_auth(&user, identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    let msg = IdentityMsg::AddMemberKey {
        new_key,
        new_kind,
        new_label,
        possession,
        authorizer,
    };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-add-member --key <path> --chain-id <id> --account-id <hex>
/// --new-key <hex> --new-kind <ed25519|p256|webauthn_p256> --nonce <n>
/// --possession <json> [--label <s>]` — the LOCAL user key (an existing
/// member) consents to admitting `new-key`; `--possession` is that key's own
/// proof (from `user-sign-possession`, or the FIDO2 transport for a passkey).
/// prints the ready-to-submit `IdentityMsg::AddMemberKey` JSON.
fn cmd_user_sign_add_member(
    args: AddMemberArgs,
    stdin: &mut impl std::io::BufRead,
) -> CommandResult {
    println!("{}", user_sign_add_member(args, stdin)?);
    Ok(())
}

/// `user-sign-remove-member` core — see [`cmd_user_sign_remove_member`].
fn user_sign_remove_member(
    args: RemoveMemberArgs,
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let account_id = config::unhex(&args.account_id)?;
    let target_key = config::unhex(&args.target_key)?;

    let user = load_user_signer(&args.key, stdin)?;
    let preimage =
        identity::remove_member_preimage(&args.chain_id, &account_id, &target_key, args.nonce);
    let authorizer =
        config::ed25519_member_auth(&user, identity::IDENTITY_REMOVE_MEMBER_NS, &preimage);
    let msg = IdentityMsg::RemoveMemberKey {
        target_key,
        authorizer,
    };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-remove-member --key <path> --chain-id <id> --account-id <hex>
/// --target-key <hex> --nonce <n>` — the LOCAL user key (a member) evicts
/// `target-key` from the account. prints the ready-to-submit
/// `IdentityMsg::RemoveMemberKey` JSON. any member may remove any member
/// except the last one.
fn cmd_user_sign_remove_member(
    args: RemoveMemberArgs,
    stdin: &mut impl std::io::BufRead,
) -> CommandResult {
    println!("{}", user_sign_remove_member(args, stdin)?);
    Ok(())
}

/// `user-webauthn-challenge` core — see [`cmd_user_webauthn_challenge`].
fn user_webauthn_challenge(args: EnrollArgs) -> Result<String, Box<dyn std::error::Error>> {
    use base64::Engine as _;

    let account_id = config::unhex(&args.account_id)?;
    let new_key = config::unhex(&args.new_key)?;

    // the exact bytes the on-chain verifier will demand the passkey signed:
    // SHA256(ADD_MEMBER_NS ‖ add_member_preimage(...)). one source of truth
    // with `identity::verify_authority` — no drift between enroll and verify.
    let preimage = identity::add_member_preimage(
        &args.chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::WebauthnP256,
        args.nonce,
    );
    let challenge = identity::webauthn_challenge(identity::IDENTITY_ADD_MEMBER_NS, &preimage);
    // base64url (no pad) — WebAuthn's native challenge encoding, so the phone
    // page passes it straight into `navigator.credentials.get({ challenge })`.
    Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(challenge))
}

/// `user-webauthn-challenge --chain-id <id> --account-id <hex> --new-key <hex>
/// --nonce <n>` — print the base64url WebAuthn challenge a passkey must sign to
/// join `account-id` as `new-key` at `nonce`. Pure computation (no key, no
/// signing): the phone's `get()` signs this, and the resulting assertion feeds
/// `user-sign-add-member --possession`. Keeping the preimage math in the node
/// (not the web page) is why "core in node" — the page never reconstructs it.
fn cmd_user_webauthn_challenge(args: EnrollArgs) -> CommandResult {
    println!("{}", user_webauthn_challenge(args)?);
    Ok(())
}

/// `user-p256-payload` core — see [`cmd_user_p256_payload`].
fn user_p256_payload(args: EnrollArgs) -> Result<String, Box<dyn std::error::Error>> {
    let account_id = config::unhex(&args.account_id)?;
    let new_key = config::unhex(&args.new_key)?;

    // the exact bytes a P256 joiner must ECDSA-sign — union_unique(ADD_MEMBER_NS,
    // add_member_preimage(...)), what the on-chain verifier reconstructs. Hex so
    // the phone hex-decodes and signs them raw; no preimage math on the page.
    let payload = identity::add_member_signing_payload(
        &args.chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::P256,
        args.nonce,
    );
    Ok(payload.iter().map(|b| format!("{b:02x}")).collect())
}

/// `user-p256-payload --chain-id <id> --account-id <hex> --new-key <hex>
/// --nonce <n>` — print the hex bytes a software P256 key (a phone's pure-JS
/// signer, in the in-app LAN enrollment) must ECDSA-P256-SHA256-sign to join
/// `account-id` as `new-key` at `nonce`. Its raw R‖S signature feeds
/// `user-sign-add-member --new-kind p256 --possession`. Pure computation.
fn cmd_user_p256_payload(args: EnrollArgs) -> CommandResult {
    println!("{}", user_p256_payload(args)?);
    Ok(())
}

#[cfg(test)]
mod webauthn_challenge_tests {
    use super::*;

    fn challenge(chain: &str, account_hex: &str, new_hex: &str, nonce: u64) -> String {
        user_webauthn_challenge(EnrollArgs {
            chain_id: chain.to_string(),
            account_id: account_hex.to_string(),
            new_key: new_hex.to_string(),
            nonce,
        })
        .unwrap()
    }

    #[test]
    fn challenge_matches_the_on_chain_verifier_math() {
        use base64::Engine as _;
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let got = challenge("team#abcd", &account_hex, &new_hex, 5);

        // recompute via identity's PUBLIC surface — the exact functions the
        // verifier uses. if the verb and the verifier ever diverge, an enrolled
        // passkey would sign a challenge the chain then rejects.
        let expected =
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(identity::webauthn_challenge(
                identity::IDENTITY_ADD_MEMBER_NS,
                &identity::add_member_preimage(
                    "team#abcd",
                    &account_id,
                    &new_key,
                    identity::KeyKind::WebauthnP256,
                    5,
                ),
            ));
        assert_eq!(got, expected);
    }

    #[test]
    fn challenge_binds_chain_account_key_and_nonce() {
        let base = challenge("c", "aa", "bb", 0);
        assert_ne!(base, challenge("d", "aa", "bb", 0), "chain must move it");
        assert_ne!(base, challenge("c", "cc", "bb", 0), "account must move it");
        assert_ne!(base, challenge("c", "aa", "cc", 0), "new key must move it");
        assert_ne!(base, challenge("c", "aa", "bb", 1), "nonce must move it");
    }

    #[test]
    fn p256_payload_matches_identity_signing_payload() {
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let got = user_p256_payload(EnrollArgs {
            chain_id: "team#abcd".to_string(),
            account_id: account_hex,
            new_key: new_hex,
            nonce: 5,
        })
        .unwrap();

        // the verb's hex must be exactly identity's signing payload — the bytes
        // the on-chain P256 verifier reconstructs.
        let expected: String = identity::add_member_signing_payload(
            "team#abcd",
            &account_id,
            &new_key,
            identity::KeyKind::P256,
            5,
        )
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
        assert_eq!(got, expected);
    }
}

#[cfg(test)]
mod userkey_verb_tests {
    use super::*;
    use std::io::Cursor;

    /// a Parser wrapper so tests can exercise the derived verb SHAPE (kebab
    /// spellings, parse rejection) the same way `main.rs`'s integrator will.
    #[derive(clap::Parser)]
    struct TestUserCli {
        #[command(subcommand)]
        #[allow(dead_code)]
        cmd: UserCmd,
    }

    /// a stdin double: one line per element, in order.
    fn stdin_of(lines: &[&str]) -> Cursor<Vec<u8>> {
        let mut s = String::new();
        for l in lines {
            s.push_str(l);
            s.push('\n');
        }
        Cursor::new(s.into_bytes())
    }

    fn empty_stdin() -> Cursor<Vec<u8>> {
        Cursor::new(Vec::new())
    }

    const TEST_PASSWORD: &str = "test password";

    fn write_encrypted(path: &std::path::Path, seed: &[u8; 32]) {
        let line = userkey::seal_user_key(seed, TEST_PASSWORD).unwrap();
        userkey::write_user_key_new(path, &line).unwrap();
    }

    fn pubkey_of(seed: &[u8; 32]) -> String {
        hex_bytes(
            ed25519::PrivateKey::decode(seed.as_slice())
                .unwrap()
                .public_key()
                .as_ref(),
        )
    }

    /// encode a one-validator test network's bearer invite blob for `role`.
    fn test_blob(issuer: &ed25519::PrivateKey, role: config::InviteRole) -> String {
        let mut d = config::NetworkDescriptor {
            chain_id: "ducktape#a1b2c3d4".into(),
            scheme: config::SCHEME_ED25519.into(),
            validators: vec![hex_bytes(issuer.public_key().as_ref())],
            bootstrap: vec![],
            reach: vec![],
            coordination: None,
        };
        d.add_bootstrap(&issuer.public_key(), "127.0.0.1:52200");
        let binding = d.genesis_namespace();
        let token = config::mint_invite_token(issuer, binding.as_bytes(), role, u64::MAX);
        // The WireGuard bootstrap is mandatory — use a minimal coordinated one.
        let wg = config::InviteWireGuard {
            public_key: [0u8; 32],
            endpoint: None,
            intro: None,
            mesh_port: 52200,
        };
        config::encode_invite(&d, &token, &wg, &[], issuer).expect("encode blob")
    }

    /// clap derives every verb's kebab name from its CamelCase variant; pin
    /// the exact prior spellings so a rename can't silently break a caller.
    #[test]
    fn verb_names_keep_prior_kebab_spelling() {
        use clap::CommandFactory as _;
        let cmd = TestUserCli::command();
        for name in [
            "key",
            "sign-bind",
            "sign-unbind",
            "sign-possession",
            "sign-add-member",
            "sign-remove-member",
            "sign-gateway-route",
            "sign-frame",
            "sign-admin",
            "redeem-invite",
            "webauthn-challenge",
            "p256-payload",
        ] {
            assert!(cmd.find_subcommand(name).is_some(), "verb {name} missing");
        }
    }

    #[test]
    fn redeem_node_prefers_explicit_node_over_network() {
        // --node short-circuits before the registry, so a bogus -n never
        // resolves; the trailing slash is trimmed.
        assert_eq!(
            redeem_node(Some("http://explicit:8844/"), Some("no-such-workspace")).unwrap(),
            "http://explicit:8844"
        );
    }

    #[test]
    fn redeem_refuses_a_resident_blob_by_name() {
        let dir = tempfile::tempdir().unwrap();
        let issuer = ed25519::PrivateKey::from_seed(1);
        let blob = test_blob(&issuer, config::InviteRole::Resident);
        let key = dir.path().join("user.key");
        let err = user_redeem_invite(
            RedeemArgs {
                blob,
                node: Some("http://127.0.0.1:1".to_string()),
                network: None,
                key,
            },
            &mut empty_stdin(),
        )
        .unwrap_err();
        assert!(
            err.to_string().contains("use `ducktape node join`"),
            "{err}"
        );
    }

    #[test]
    fn init_writes_encrypted_v1_and_outputs_mnemonic_then_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut stdin = stdin_of(&["correct horse battery"]);

        let (words, pubkey_hex) =
            user_key_init(KeyOutArgs { out: path.clone() }, &mut stdin).unwrap();

        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(pubkey_hex.len(), 64);
        let enc = userkey::read_user_key_file(&path).unwrap();
                assert_eq!(hex_bytes(&enc.pubkey), pubkey_hex);
            }

    #[test]
    fn restore_round_trips_init_mnemonic_to_identical_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let init_path = dir.path().join("a.key");
        let mut init_stdin = stdin_of(&["password one"]);
        let (words, pubkey_hex) =
            user_key_init(KeyOutArgs { out: init_path }, &mut init_stdin).unwrap();

        let restore_path = dir.path().join("b.key");
        let mut restore_stdin = stdin_of(&[&words, "password two"]);
        let restored_pubkey =
            user_key_restore(KeyOutArgs { out: restore_path }, &mut restore_stdin).unwrap();

        assert_eq!(restored_pubkey, pubkey_hex);
    }

    #[test]
    fn unlock_verifies_and_rejects_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut init_stdin = stdin_of(&["right password"]);
        let (_, pubkey_hex) =
            user_key_init(KeyOutArgs { out: path.clone() }, &mut init_stdin).unwrap();

        let mut ok_stdin = stdin_of(&["right password"]);
        let unlocked = user_key_unlock(KeyPathArgs { key: path.clone() }, &mut ok_stdin).unwrap();
        assert_eq!(unlocked, pubkey_hex);

        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(user_key_unlock(KeyPathArgs { key: path }, &mut bad_stdin).is_err());
    }

    #[test]
    fn reveal_returns_the_init_mnemonic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut init_stdin = stdin_of(&["a password"]);
        let (words, _) = user_key_init(KeyOutArgs { out: path.clone() }, &mut init_stdin).unwrap();
        let mut reveal_stdin = stdin_of(&["a password"]);
        let revealed = user_key_reveal(KeyPathArgs { key: path }, &mut reveal_stdin).unwrap();
        assert_eq!(revealed, words);
    }

    #[test]
    fn status_reports_absent_or_encrypted_and_rejects_bare_hex() {
        let dir = tempfile::tempdir().unwrap();

        let absent_path = dir.path().join("absent.key");
        assert_eq!(
            user_key_status(KeyPathArgs { key: absent_path }).unwrap(),
            "absent"
        );

        let bare_path = dir.path().join("bare.key");
        let seed = [3u8; 32];
        userkey::write_user_key_new(&bare_path, &hex_bytes(&seed)).unwrap();
        assert!(user_key_status(KeyPathArgs { key: bare_path }).is_err());

        let encrypted_path = dir.path().join("encrypted.key");
        let mut stdin = stdin_of(&["a password"]);
        let (_, init_pubkey_hex) = user_key_init(
            KeyOutArgs {
                out: encrypted_path.clone(),
            },
            &mut stdin,
        )
        .unwrap();
        assert_eq!(
            user_key_status(KeyPathArgs {
                key: encrypted_path
            })
            .unwrap(),
            format!("encrypted {init_pubkey_hex}")
        );
    }

    #[test]
    fn short_password_rejected_in_init_and_restore() {
        let dir = tempfile::tempdir().unwrap();

        let init_path = dir.path().join("init.key");
        let mut stdin = stdin_of(&["short1"]);
        assert!(
            user_key_init(
                KeyOutArgs {
                    out: init_path.clone()
                },
                &mut stdin
            )
            .is_err()
        );
        assert!(
            !init_path.exists(),
            "a rejected password must not write a file"
        );

        let words = userkey::mnemonic_of_seed(&[1u8; 32]);
        let restore_path = dir.path().join("restore.key");
        let mut stdin = stdin_of(&[&words, "short1"]);
        assert!(
            user_key_restore(
                KeyOutArgs {
                    out: restore_path.clone()
                },
                &mut stdin
            )
            .is_err()
        );
        assert!(!restore_path.exists());
    }

    #[test]
    fn sign_bind_decodes_and_wrong_password_fails() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [77u8; 32];
        // an arbitrary but VALID ed25519 point — a public key derived from a
        // seed, not raw bytes (not every 32-byte string is on-curve).
        let node_pub_hex = pubkey_of(&[100u8; 32]);

        let key_path = dir.path().join("user.key");
        let line = userkey::seal_user_key(&seed, "a password").unwrap();
        userkey::write_user_key_new(&key_path, &line).unwrap();
        let mut stdin = stdin_of(&["a password"]);
        let json = user_sign_bind(
            NodeBindArgs {
                key: key_path.clone(),
                chain_id: "test-chain".to_string(),
                node_pub: node_pub_hex.clone(),
                nonce: 0,
            },
            &mut stdin,
        )
        .unwrap();

        match identity::decode_msg(json.as_bytes()).unwrap() {
            identity::IdentityMsg::BindNode { authorizer } => {
                assert_eq!(authorizer.key, pubkey_bytes(&seed));
                assert_eq!(authorizer.kind, identity::KeyKind::Ed25519);
            }
            other => panic!("expected BindNode, got {other:?}"),
        }

        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(
            user_sign_bind(
                NodeBindArgs {
                    key: key_path,
                    chain_id: "test-chain".to_string(),
                    node_pub: node_pub_hex,
                    nonce: 0,
                },
                &mut bad_stdin,
            )
            .is_err()
        );
    }

    #[test]
    fn sign_gateway_route_is_namespace_scoped_strict_and_decodable() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [81u8; 32];
        let key_path = dir.path().join("user.key");
        write_encrypted(&key_path, &seed);
        let signer = ed25519::PrivateKey::decode(seed.as_slice()).unwrap();
        let statement = gateway::RouteStatement {
            version: 1,
            chain_id: "test-chain".into(),
            account_id: signer.public_key().as_ref().to_vec(),
            name: gateway::RouteName::named("api"),
            publisher_node: pubkey_bytes(&[82u8; 32]),
            revision: 1,
            route: Some(gateway::RouteDefinition {
                target: gateway::RouteTarget::LoopbackHttp,
                policy: gateway::RoutePolicy {
                    audience: gateway::RouteAudience::Network,
                    methods: vec![gateway::RouteMethod::Get, gateway::RouteMethod::Post],
                    max_request_bytes: 1024,
                    max_response_bytes: 4096,
                    allow_authorization: false,
                    allow_upgrade: false,
                },
            }),
        };
        let mut stdin = stdin_of(&[TEST_PASSWORD]);
        let json = user_sign_gateway_route(
            GatewayRouteArgs {
                key: key_path.clone(),
                statement: serde_json::to_string(&statement).unwrap(),
            },
            &mut stdin,
        )
        .unwrap();
        let gateway::GatewayMsg::SetRoute {
            statement: decoded,
            authorization,
        } = gateway::decode_msg(json.as_bytes()).unwrap()
        else {
            panic!("userkey mints a SetRoute");
        };
        assert_eq!(decoded, statement);
        assert_eq!(authorization.signer, signer.public_key().as_ref());
        assert!(identity::verify_authority(
            identity::KeyKind::Ed25519,
            &authorization.signer,
            None,
            gateway::GATEWAY_ROUTE_NS,
            &gateway::route_signing_preimage(&decoded).unwrap(),
            &identity::MemberProof::Signature {
                sig: authorization.signature,
            },
        ));

        let mut unsafe_statement = statement;
        unsafe_statement.name = gateway::RouteName::named("Api.Evil");
        let mut stdin = stdin_of(&[TEST_PASSWORD]);
        assert!(
            user_sign_gateway_route(
                GatewayRouteArgs {
                    key: key_path,
                    statement: serde_json::to_string(&unsafe_statement).unwrap(),
                },
                &mut stdin,
            )
            .is_err()
        );
    }

    #[test]
    fn sign_unbind_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [88u8; 32];
        let node_pub_hex = pubkey_of(&[101u8; 32]);

        let key_path = dir.path().join("user.key");
        write_encrypted(&key_path, &seed);
        let mut stdin = stdin_of(&[TEST_PASSWORD]);
        let json = user_sign_unbind(
            NodeBindArgs {
                key: key_path,
                chain_id: "test-chain".to_string(),
                node_pub: node_pub_hex,
                nonce: 1,
            },
            &mut stdin,
        )
        .unwrap();

        assert!(identity::decode_msg(json.as_bytes()).is_ok());
    }

    fn pubkey_bytes(seed: &[u8; 32]) -> Vec<u8> {
        ed25519::PrivateKey::decode(seed.as_slice())
            .unwrap()
            .public_key()
            .as_ref()
            .to_vec()
    }

    #[test]
    fn unknown_key_subcommand_is_rejected_at_parse() {
        // clap now owns the rejection an unknown `user key <x>` used to hit in
        // the hand dispatch: an unexpected token under `key` fails to parse.
        use clap::Parser as _;
        assert!(TestUserCli::try_parse_from(["user", "key", "bogus"]).is_err());
        assert!(TestUserCli::try_parse_from(["user", "key"]).is_err());
    }

    #[test]
    fn sign_frame_round_trips_through_decode_frame() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        write_encrypted(&key_path, &[7u8; 32]);

        let payload: &[u8] = b"\x00raw chunk bytes";
        let mut stdin = stdin_of(&[TEST_PASSWORD, &hex_bytes(payload)]);
        let frame_hex = user_sign_frame(
            FrameArgs {
                key: key_path,
                target: "files".to_string(),
                seq: 42,
            },
            &mut stdin,
        )
        .unwrap();

        let (origin, msg, _cont) =
            node::decode_frame(&config::unhex(&frame_hex).unwrap()).expect("frame verifies");
        let signer = ed25519::PrivateKey::decode([7u8; 32].as_slice()).unwrap();
        assert_eq!(
            origin,
            sdk::Origin::External(signer.public_key().as_ref().to_vec())
        );
        assert_eq!(msg.target, "files");
        assert_eq!(msg.payload, payload);
    }

    #[test]
    fn sign_admin_returns_owner_pop_the_verifier_would_accept() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        write_encrypted(&key_path, &[7u8; 32]);

        let node_key = [0xabu8; 32];
        let mut stdin = stdin_of(&[TEST_PASSWORD]);
        let out = user_sign_admin(
            AdminArgs {
                key: key_path,
                method: "POST".to_string(),
                path: "/v1/admin/shutdown".to_string(),
                node_key: hex_bytes(&node_key),
            },
            &mut stdin,
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&out).expect("one json line");
        let signer = ed25519::PrivateKey::decode([7u8; 32].as_slice()).unwrap();
        // the reported key is the user's own account key.
        assert_eq!(parsed["key"], hex_bytes(signer.public_key().as_ref()));
        // the signature is exactly what the node's verifier reconstructs over
        // the SAME (method, path, node_key, ts) — proving the verb signed the
        // right bytes with the right key (one source: noded::admin::sign_admin).
        let ts: u64 = parsed["ts"].as_str().unwrap().parse().unwrap();
        let expect = noded::admin::sign_admin(&signer, "POST", "/v1/admin/shutdown", &node_key, ts);
        assert_eq!(parsed["sig"], hex_bytes(expect.as_ref()));
        // and it is node-bound: the same tuple against a different node differs.
        let other =
            noded::admin::sign_admin(&signer, "POST", "/v1/admin/shutdown", &[0xcd; 32], ts);
        assert_ne!(parsed["sig"], hex_bytes(other.as_ref()));
    }

    #[test]
    fn sign_frame_reads_the_password_first_for_encrypted_keys() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        let line = userkey::seal_user_key(&[9u8; 32], "hunter2duck").unwrap();
        userkey::write_user_key_new(&key_path, &line).unwrap();

        let mut stdin = stdin_of(&["hunter2duck", &hex_bytes(b"{}")]);
        let frame_hex = user_sign_frame(
            FrameArgs {
                key: key_path.clone(),
                target: "files".to_string(),
                seq: 0,
            },
            &mut stdin,
        )
        .unwrap();
        assert!(node::decode_frame(&config::unhex(&frame_hex).unwrap()).is_ok());

        // wrong password: refused before any payload is read.
        let mut stdin = stdin_of(&["wrong password", &hex_bytes(b"{}")]);
        assert!(
            user_sign_frame(
                FrameArgs {
                    key: key_path,
                    target: "files".to_string(),
                    seq: 0,
                },
                &mut stdin,
            )
            .is_err()
        );
    }

    /// The tty wrapper's non-tty path must be a transparent pass-through: no
    /// prompt, no termios, and it forwards the inner read's value AND its EOF
    /// error verbatim — the piped-stdin contract the sign/key verbs rely on.
    /// (The tty path toggles the real terminal and is untestable headless.)
    #[test]
    fn with_prompt_non_tty_is_a_bare_read() {
        let mut piped = stdin_of(&["hunter2duck"]);
        let got = with_prompt("password", false, || {
            read_stdin_line(&mut piped, "password")
        })
        .unwrap();
        assert_eq!(got, "hunter2duck");

        // a secret field name doesn't change the non-tty behavior either.
        assert!(field_is_secret("password") && field_is_secret("mnemonic"));
        assert!(!field_is_secret("payload-hex"));

        // EOF still errors, exactly as a bare read_stdin_line would.
        let mut empty = empty_stdin();
        assert!(
            with_prompt("password", false, || read_stdin_line(
                &mut empty, "password"
            ))
            .is_err()
        );
    }
}
