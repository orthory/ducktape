//! User identity lifecycle and signing command surface.
//!
//! These commands are synchronous operator tooling. Keeping them outside the
//! live node entrypoint makes their stdin-only secret handling and output
//! contracts independently testable without mixing them into runtime boot.

use std::path::PathBuf;

use commonware_codec::{DecodeExt as _, Encode as _};
use commonware_cryptography::{Signer as _, ed25519};

use crate::{cli_flags::parse_flags, config, userkey};
use config::hex_bytes;

type CommandResult = Result<(), Box<dyn std::error::Error>>;

/// Run a user-identity command, or return `None` when `command` belongs to a
/// different command family.
pub(super) fn dispatch(command: &str, args: &[String]) -> Option<CommandResult> {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let result = match command {
        "user-key" => cmd_user_key(args, &mut stdin),
        "user-sign-bind" => cmd_user_sign_bind(args, &mut stdin),
        "user-sign-unbind" => cmd_user_sign_unbind(args, &mut stdin),
        "user-sign-possession" => cmd_user_sign_possession(args, &mut stdin),
        "user-sign-add-member" => cmd_user_sign_add_member(args, &mut stdin),
        "user-sign-remove-member" => cmd_user_sign_remove_member(args, &mut stdin),
        "user-sign-gateway-route" => cmd_user_sign_gateway_route(args, &mut stdin),
        "user-sign-frame" => cmd_user_sign_frame(args, &mut stdin),
        "user-sign-admin" => cmd_user_sign_admin(args, &mut stdin),
        "user-webauthn-challenge" => cmd_user_webauthn_challenge(args),
        "user-p256-payload" => cmd_user_p256_payload(args),
        _ => return None,
    };
    Some(result)
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

/// the design spec's floor for NEW passwords (`init`/`restore`/`encrypt`),
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

/// `path`'s raw trimmed line — the exact text [`userkey::open_user_key`]
/// parses, as opposed to [`userkey::read_user_key_file`]'s already-decoded
/// shape. verbs that must hand a v2 line to `open_user_key` read it via this.
fn read_key_line(path: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read {path:?}: {e}"))?;
    let line = text.trim();
    if line.is_empty() {
        return Err(format!("{path:?} is empty"));
    }
    Ok(line.to_string())
}

/// resolve the USER signer at `key_path` for the sign verbs: a v2
/// (encrypted) file decrypts with a password read as the FIRST stdin line;
/// anything else (legacy plaintext, or absent — freshly generated) falls
/// through to [`config::load_or_generate_identity`] UNCHANGED, reading no
/// stdin at all — byte-identical to the pre-onboarding sign-verb behavior.
fn load_user_signer(
    key_path: &std::path::Path,
    stdin: &mut impl std::io::BufRead,
) -> Result<ed25519::PrivateKey, Box<dyn std::error::Error>> {
    if let Ok(text) = std::fs::read_to_string(key_path)
        && text.trim().starts_with(userkey::USER_KEY_V2_PREFIX)
    {
        let password = read_stdin_line(stdin, "password")?;
        return Ok(userkey::open_user_key(text.trim(), &password)?);
    }
    let (user, generated) = config::load_or_generate_identity(key_path)?;
    if generated {
        eprintln!("generated user identity at {}", key_path.display());
    }
    Ok(user)
}

/// `user-key init` core — see [`cmd_user_key_init`] for the print contract.
/// returns `(mnemonic, pubkey-hex)` so tests can assert both independently.
fn user_key_init(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let mut seed = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::rngs::OsRng, &mut seed);
    let words = userkey::mnemonic_of_seed(&seed);
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice()).expect("32 random bytes decode");
    Ok((words, hex_bytes(key.public_key().as_ref())))
}

/// `user-key init --out <path>` — stdin: password. Generates a fresh seed,
/// writes v2 (refuses to overwrite via `create_new`), and prints the 24-word
/// mnemonic line THEN the pubkey-hex line — pubkey is the LAST stdout line
/// (the `run_verb`/`last_line` contract), mnemonic is the line before it.
fn cmd_user_key_init(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    let (words, pubkey_hex) = user_key_init(args, stdin)?;
    println!("{words}");
    println!("{pubkey_hex}");
    Ok(())
}

/// `user-key restore` core — see [`cmd_user_key_restore`].
fn user_key_restore(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let mnemonic = read_stdin_line(stdin, "mnemonic")?;
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let seed = userkey::seed_of_mnemonic(&mnemonic)?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::write_user_key_new(&out, &line)?;

    let key = ed25519::PrivateKey::decode(seed.as_slice())
        .map_err(|e| format!("restored seed is not a valid ed25519 secret: {e}"))?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key restore --out <path>` — stdin: mnemonic line, then password
/// line. Validates the BIP39 checksum, writes v2 (refuses to overwrite),
/// prints the pubkey (the only stdout line).
fn cmd_user_key_restore(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_restore(args, stdin)?);
    Ok(())
}

/// `user-key unlock` core — see [`cmd_user_key_unlock`].
fn user_key_unlock(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-key unlock needs --key <path>")?,
    );
    let password = read_stdin_line(stdin, "password")?;

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            let line = read_key_line(&key_path)?;
            userkey::open_user_key(&line, &password)?
        }
    };
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key unlock --key <path>` — stdin: password. Pure verification
/// (nothing persists); prints the pubkey on success, a clean error + nonzero
/// exit on a wrong password or a corrupt file.
fn cmd_user_key_unlock(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_unlock(args, stdin)?);
    Ok(())
}

/// `user-key reveal` core — see [`cmd_user_key_reveal`].
fn user_key_reveal(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-key reveal needs --key <path>")?,
    );

    // legacy plaintext tolerates an absent/empty password line; only an
    // encrypted file actually needs one. read leniently (empty on EOF) so a
    // caller revealing a legacy key doesn't have to pipe an unused line.
    let mut password = String::new();
    let _ = stdin.read_line(&mut password);
    while password.ends_with('\n') || password.ends_with('\r') {
        password.pop();
    }

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            let line = read_key_line(&key_path)?;
            userkey::open_user_key(&line, &password)?
        }
    };
    let seed_bytes = key.encode();
    let seed: [u8; 32] = seed_bytes
        .as_ref()
        .try_into()
        .map_err(|_| "decoded key is not a 32-byte seed".to_string())?;
    Ok(userkey::mnemonic_of_seed(&seed))
}

/// `user-key reveal --key <path>` — stdin: password (empty/absent tolerated
/// for legacy plaintext, required to decrypt v2). Prints the 24-word
/// mnemonic — the SAME encoding `init`/`restore` use, so it round-trips
/// through `user-key restore` to the identical pubkey.
fn cmd_user_key_reveal(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_reveal(args, stdin)?);
    Ok(())
}

/// `user-key encrypt` core — see [`cmd_user_key_encrypt`].
fn user_key_encrypt(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-key encrypt needs --key <path>")?,
    );
    let password = read_stdin_line(stdin, "password")?;
    check_password_len(&password)?;

    let key = match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => key,
        userkey::UserKeyFile::Encrypted(_) => {
            return Err(format!("{} is already encrypted", key_path.display()).into());
        }
    };
    let seed_bytes = key.encode();
    let seed: [u8; 32] = seed_bytes
        .as_ref()
        .try_into()
        .map_err(|_| "decoded key is not a 32-byte seed".to_string())?;
    let line = userkey::seal_user_key(&seed, &password)?;
    userkey::rewrite_user_key(&key_path, &line)?;
    Ok(hex_bytes(key.public_key().as_ref()))
}

/// `user-key encrypt --key <path>` — stdin: password. Migrates a legacy v1
/// plaintext file to v2 in place (temp file + rename, the same atomicity as
/// every other in-place rewrite); errors (no-op) if the file is already v2.
/// Prints the pubkey (unchanged by the migration).
fn cmd_user_key_encrypt(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_encrypt(args, stdin)?);
    Ok(())
}

/// `user-key status` core — see [`cmd_user_key_status`].
fn user_key_status(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-key status needs --key <path>")?,
    );
    if !key_path.exists() {
        return Ok("absent".to_string());
    }
    Ok(match userkey::read_user_key_file(&key_path)? {
        userkey::UserKeyFile::Plaintext(key) => {
            format!("plaintext {}", hex_bytes(key.public_key().as_ref()))
        }
        userkey::UserKeyFile::Encrypted(enc) => format!("encrypted {}", hex_bytes(&enc.pubkey)),
    })
}

/// `user-key status --key <path>` — no stdin. Prints exactly one of `absent`
/// | `plaintext <pubkey-hex>` | `encrypted <pubkey-hex>`; never touches a
/// password, so it's safe to poll from the app on every launch.
fn cmd_user_key_status(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_key_status(args)?);
    Ok(())
}

/// `user-key [init|restore|unlock|reveal|encrypt|status]` — dispatches to the
/// v2 lifecycle verbs (see the design spec's "CLI verbs" section); a bare
/// `user-key [--out <path>]` (no recognized subcommand) falls through to the
/// legacy v1 generate-or-reuse shape from #205, kept working unchanged for
/// the app/tests until the app migrates onto the v2 verbs.
fn cmd_user_key(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("init") => return cmd_user_key_init(&args[1..], stdin),
        Some("restore") => return cmd_user_key_restore(&args[1..], stdin),
        Some("unlock") => return cmd_user_key_unlock(&args[1..], stdin),
        Some("reveal") => return cmd_user_key_reveal(&args[1..], stdin),
        Some("encrypt") => return cmd_user_key_encrypt(&args[1..], stdin),
        Some("status") => return cmd_user_key_status(&args[1..]),
        Some(other) if !other.starts_with("--") => {
            return Err(format!(
                "unknown user-key subcommand {other:?} (want \
                 init|restore|unlock|reveal|encrypt|status, or a bare \
                 `user-key [--out <path>]` to generate/reuse a legacy key)"
            )
            .into());
        }
        _ => {}
    }
    cmd_user_key_generate_legacy(args)
}

/// `user-key [--out <path>]` — generate (or reuse) a persisted ed25519 USER
/// identity: the human's app-side keypair (distinct from `keygen`'s per-node
/// identity), a bare hex ed25519 seed file under the same load-or-generate
/// discipline. pubkey on stdout (scriptable — the desktop shell's `run_verb`
/// takes the LAST stdout line as the value), provenance on stderr. the
/// legacy v1 shape (#205), kept working verbatim; `init` is the v2
/// replacement `cmd_user_key` dispatches to instead.
fn cmd_user_key_generate_legacy(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let out = PathBuf::from(flags.get("out").map(String::as_str).unwrap_or("user.key"));
    let (key, generated) = config::load_or_generate_identity(&out)?;
    println!("{}", hex_bytes(key.public_key().as_ref()));
    eprintln!(
        "{} user identity at {}",
        if generated { "generated" } else { "reusing" },
        out.display()
    );
    Ok(())
}

/// `user-sign-bind` core — see [`cmd_user_sign_bind`].
fn user_sign_bind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-bind needs --key <path>")?,
    );
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-bind needs --chain-id <id>")?;
    let node_pub_hex = flags
        .get("node-pub")
        .ok_or("user-sign-bind needs --node-pub <hex>")?;
    let node_pub = config::decode_key(node_pub_hex)?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-bind needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_BIND_NS,
        &identity::bind_preimage(chain_id, node_pub.as_ref(), nonce),
    );
    let msg = IdentityMsg::BindNode { authorizer };
    Ok(String::from_utf8(encode_msg(&msg)).expect("json is utf-8"))
}

/// `user-sign-bind --key <path> --chain-id <id> --node-pub <hex> --nonce <n>`
/// — mint a bind certificate binding `node-pub` to the user identity at
/// `--key` (generated there if absent, or decrypted with stdin's password
/// line if it's a v2 file — see [`load_user_signer`]), at `chain-id`/`nonce`,
/// and print the ready-to-submit `IdentityMsg::BindNode` JSON as the last
/// (only) stdout line. `user_key` rides the payload — the node being bound is
/// the verified submit ORIGIN, never a payload field; the module resolves it
/// from the rpc transport, not from this CLI.
fn cmd_user_sign_bind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_bind(args, stdin)?);
    Ok(())
}

/// `user-sign-unbind` core — see [`cmd_user_sign_unbind`].
fn user_sign_unbind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-unbind needs --key <path>")?,
    );
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-unbind needs --chain-id <id>")?;
    let node_pub_hex = flags
        .get("node-pub")
        .ok_or("user-sign-unbind needs --node-pub <hex>")?;
    let node_pub = config::decode_key(node_pub_hex)?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-unbind needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let authorizer = config::ed25519_member_auth(
        &user,
        identity::IDENTITY_UNBIND_NS,
        &identity::unbind_preimage(chain_id, node_pub.as_ref(), nonce),
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
fn cmd_user_sign_unbind(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_unbind(args, stdin)?);
    Ok(())
}

/// Sign one canonical gateway route and return a ready-to-submit `GatewayMsg`.
/// This remains a namespace-specific oracle: the CLI parses and validates the
/// complete bounded route before the user key signs it.
fn user_sign_gateway_route(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-gateway-route needs --key <path>")?,
    );
    let statement_json = flags
        .get("statement")
        .ok_or("user-sign-gateway-route needs --statement <json>")?;
    if statement_json.len() > gateway::MAX_ROUTE_STATEMENT_JSON_BYTES {
        return Err("user-sign-gateway-route statement exceeds the byte cap".into());
    }
    let statement: gateway::RouteStatement = serde_json::from_str(statement_json)?;
    let preimage = gateway::route_signing_preimage(&statement)?;
    let user = load_user_signer(&key_path, stdin)?;
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_gateway_route(args, stdin)?);
    Ok(())
}

/// `user-sign-frame` core — see [`cmd_user_sign_frame`].
fn user_sign_frame(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-frame needs --key <path>")?,
    );
    let target = flags
        .get("target")
        .ok_or("user-sign-frame needs --target <module>")?
        .clone();
    let seq: u64 = flags
        .get("seq")
        .ok_or("user-sign-frame needs --seq <n>")?
        .parse()
        .map_err(|e| format!("--seq is not a valid u64: {e}"))?;

    // stdin order: password FIRST (only when the key file is v2-encrypted —
    // load_user_signer reads nothing otherwise), then the payload as ONE hex
    // line. the payload is not a secret; it rides stdin because a 1 MiB chunk
    // frame would blow past OS argv limits.
    let user = load_user_signer(&key_path, stdin)?;
    let payload_hex = read_stdin_line(stdin, "payload-hex")?;
    let payload = config::unhex(&payload_hex).map_err(|e| format!("payload hex: {e}"))?;

    let frame = node::encode_frame(&user, seq, &sdk::Msg { target, payload });
    Ok(hex_bytes(&frame))
}

/// `user-sign-frame --key <path> --target <module> --seq <n>` — stdin:
/// [password line when the key is v2-encrypted], then one payload-hex line.
/// Wraps the payload in a `node` op frame signed by the user key and prints
/// the frame as hex (the only stdout line). POSTed raw to `/v1/submit/frame`,
/// the frame's verified signer becomes the op's `Origin::External` — the
/// authenticated authorship the frameless `/v1/submit`'s plaintext `origin`
/// convention cannot provide. `seq` is the frame's ordering/dedup tie-breaker
/// (same-payload resubmits need a fresh seq to survive the consensus lane's
/// content-digest replay guard); it is NOT tracked in state — any u64 works.
fn cmd_user_sign_frame(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_frame(args, stdin)?);
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(flags.get("key").ok_or("user-sign-admin needs --key <path>")?);
    let method = flags
        .get("method")
        .ok_or("user-sign-admin needs --method <M>")?
        .clone();
    let path = flags
        .get("path")
        .ok_or("user-sign-admin needs --path <path-and-query>")?
        .clone();

    // stdin: password ONLY (when the key is v2-encrypted) — there is no payload.
    let user = load_user_signer(&key_path, stdin)?;
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let sig = noded::admin::sign_admin(&user, &method, &path, ts);
    let out = serde_json::json!({
        "key": hex_bytes(user.public_key().as_ref()),
        "ts": ts.to_string(),
        "sig": hex_bytes(sig.as_ref()),
    });
    Ok(out.to_string())
}

/// `user-sign-admin --key <path> --method <M> --path <path-and-query>` — stdin:
/// [password line when the key is v2-encrypted]. Prints one JSON line
/// `{"key","ts","sig"}` the app turns into the `x-ducktape-admin-*` headers of
/// an owner control request. Fresh `ts` per call (the PoP is replay-bounded).
fn cmd_user_sign_admin(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-possession needs --key <path>")?,
    );
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-possession needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-possession needs --account-id <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-possession needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // this key proves it holds itself over the add-member preimage; its own
    // pubkey is `new_key`, and the node's user key is ed25519.
    let user = load_user_signer(&key_path, stdin)?;
    let new_key = user.public_key().as_ref().to_vec();
    let preimage = identity::add_member_preimage(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::Ed25519,
        nonce,
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_possession(args, stdin)?);
    Ok(())
}

/// `user-sign-add-member` core — see [`cmd_user_sign_add_member`].
fn user_sign_add_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-add-member needs --key <path>")?,
    );
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-add-member needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-add-member needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-sign-add-member needs --new-key <hex>")?,
    )?;
    let new_kind = parse_kind(
        flags
            .get("new-kind")
            .ok_or("user-sign-add-member needs --new-kind <ed25519|p256|webauthn_p256>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-add-member needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;
    let new_label = flags.get("label").cloned();
    let possession: identity::MemberProof = serde_json::from_str(
        flags
            .get("possession")
            .ok_or("user-sign-add-member needs --possession <MemberProof json>")?,
    )
    .map_err(|e| format!("--possession is not a MemberProof: {e}"))?;

    // the local user key is an existing member; it consents to admitting the
    // new key over the same preimage the new key proved possession of.
    let user = load_user_signer(&key_path, stdin)?;
    let preimage = identity::add_member_preimage(chain_id, &account_id, &new_key, new_kind, nonce);
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_add_member(args, stdin)?);
    Ok(())
}

/// `user-sign-remove-member` core — see [`cmd_user_sign_remove_member`].
fn user_sign_remove_member(
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<String, Box<dyn std::error::Error>> {
    use identity::{IdentityMsg, encode_msg};

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let key_path = PathBuf::from(
        flags
            .get("key")
            .ok_or("user-sign-remove-member needs --key <path>")?,
    );
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-sign-remove-member needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-sign-remove-member needs --account-id <hex>")?,
    )?;
    let target_key = config::unhex(
        flags
            .get("target-key")
            .ok_or("user-sign-remove-member needs --target-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-sign-remove-member needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    let user = load_user_signer(&key_path, stdin)?;
    let preimage = identity::remove_member_preimage(chain_id, &account_id, &target_key, nonce);
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
    args: &[String],
    stdin: &mut impl std::io::BufRead,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_sign_remove_member(args, stdin)?);
    Ok(())
}

/// `user-webauthn-challenge` core — see [`cmd_user_webauthn_challenge`].
fn user_webauthn_challenge(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    use base64::Engine as _;

    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-webauthn-challenge needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-webauthn-challenge needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-webauthn-challenge needs --new-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-webauthn-challenge needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // the exact bytes the on-chain verifier will demand the passkey signed:
    // SHA256(ADD_MEMBER_NS ‖ add_member_preimage(...)). one source of truth
    // with `identity::verify_authority` — no drift between enroll and verify.
    let preimage = identity::add_member_preimage(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::WebauthnP256,
        nonce,
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
fn cmd_user_webauthn_challenge(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_webauthn_challenge(args)?);
    Ok(())
}

/// `user-p256-payload` core — see [`cmd_user_p256_payload`].
fn user_p256_payload(args: &[String]) -> Result<String, Box<dyn std::error::Error>> {
    let (pos, flags) = parse_flags(args)?;
    if !pos.is_empty() {
        return Err(format!("unexpected args: {pos:?}").into());
    }
    let chain_id = flags
        .get("chain-id")
        .ok_or("user-p256-payload needs --chain-id <id>")?;
    let account_id = config::unhex(
        flags
            .get("account-id")
            .ok_or("user-p256-payload needs --account-id <hex>")?,
    )?;
    let new_key = config::unhex(
        flags
            .get("new-key")
            .ok_or("user-p256-payload needs --new-key <hex>")?,
    )?;
    let nonce: u64 = flags
        .get("nonce")
        .ok_or("user-p256-payload needs --nonce <n>")?
        .parse()
        .map_err(|e| format!("--nonce is not a valid u64: {e}"))?;

    // the exact bytes a P256 joiner must ECDSA-sign — union_unique(ADD_MEMBER_NS,
    // add_member_preimage(...)), what the on-chain verifier reconstructs. Hex so
    // the phone hex-decodes and signs them raw; no preimage math on the page.
    let payload = identity::add_member_signing_payload(
        chain_id,
        &account_id,
        &new_key,
        identity::KeyKind::P256,
        nonce,
    );
    Ok(payload.iter().map(|b| format!("{b:02x}")).collect())
}

/// `user-p256-payload --chain-id <id> --account-id <hex> --new-key <hex>
/// --nonce <n>` — print the hex bytes a software P256 key (a phone's pure-JS
/// signer, in the in-app LAN enrollment) must ECDSA-P256-SHA256-sign to join
/// `account-id` as `new-key` at `nonce`. Its raw R‖S signature feeds
/// `user-sign-add-member --new-kind p256 --possession`. Pure computation.
fn cmd_user_p256_payload(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    println!("{}", user_p256_payload(args)?);
    Ok(())
}

#[cfg(test)]
mod webauthn_challenge_tests {
    use super::*;

    fn challenge(chain: &str, account_hex: &str, new_hex: &str, nonce: &str) -> String {
        let args: Vec<String> = [
            "--chain-id",
            chain,
            "--account-id",
            account_hex,
            "--new-key",
            new_hex,
            "--nonce",
            nonce,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        user_webauthn_challenge(&args).unwrap()
    }

    #[test]
    fn challenge_matches_the_on_chain_verifier_math() {
        use base64::Engine as _;
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let got = challenge("team#abcd", &account_hex, &new_hex, "5");

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
        let base = challenge("c", "aa", "bb", "0");
        assert_ne!(base, challenge("d", "aa", "bb", "0"), "chain must move it");
        assert_ne!(
            base,
            challenge("c", "cc", "bb", "0"),
            "account must move it"
        );
        assert_ne!(
            base,
            challenge("c", "aa", "cc", "0"),
            "new key must move it"
        );
        assert_ne!(base, challenge("c", "aa", "bb", "1"), "nonce must move it");
    }

    #[test]
    fn p256_payload_matches_identity_signing_payload() {
        let account_id = [0xabu8; 33];
        let new_key = [0xcdu8; 33];
        let account_hex: String = account_id.iter().map(|b| format!("{b:02x}")).collect();
        let new_hex: String = new_key.iter().map(|b| format!("{b:02x}")).collect();

        let args: Vec<String> = [
            "--chain-id",
            "team#abcd",
            "--account-id",
            &account_hex,
            "--new-key",
            &new_hex,
            "--nonce",
            "5",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        let got = user_p256_payload(&args).unwrap();

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

    /// build the `&[String]` verb args from string-literal parts.
    fn args_of(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
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

    fn write_legacy(path: &std::path::Path, seed: &[u8; 32]) {
        userkey::write_user_key_new(path, &hex_bytes(seed)).unwrap();
    }

    fn pubkey_of(seed: &[u8; 32]) -> String {
        hex_bytes(
            ed25519::PrivateKey::decode(seed.as_slice())
                .unwrap()
                .public_key()
                .as_ref(),
        )
    }

    #[test]
    fn init_writes_v2_and_outputs_mnemonic_then_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut stdin = stdin_of(&["correct horse battery"]);

        let (words, pubkey_hex) =
            user_key_init(&args_of(&["--out", &path.to_string_lossy()]), &mut stdin).unwrap();

        assert_eq!(words.split_whitespace().count(), 24);
        assert_eq!(pubkey_hex.len(), 64);
        match userkey::read_user_key_file(&path).unwrap() {
            userkey::UserKeyFile::Encrypted(enc) => {
                assert_eq!(hex_bytes(&enc.pubkey), pubkey_hex);
            }
            userkey::UserKeyFile::Plaintext(_) => panic!("expected v2/Encrypted"),
        }
    }

    #[test]
    fn restore_round_trips_init_mnemonic_to_identical_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let init_path = dir.path().join("a.key");
        let mut init_stdin = stdin_of(&["password one"]);
        let (words, pubkey_hex) = user_key_init(
            &args_of(&["--out", &init_path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();

        let restore_path = dir.path().join("b.key");
        let mut restore_stdin = stdin_of(&[&words, "password two"]);
        let restored_pubkey = user_key_restore(
            &args_of(&["--out", &restore_path.to_string_lossy()]),
            &mut restore_stdin,
        )
        .unwrap();

        assert_eq!(restored_pubkey, pubkey_hex);
    }

    #[test]
    fn unlock_verifies_and_rejects_wrong_password() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let mut init_stdin = stdin_of(&["right password"]);
        let (_, pubkey_hex) = user_key_init(
            &args_of(&["--out", &path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();

        let mut ok_stdin = stdin_of(&["right password"]);
        let unlocked =
            user_key_unlock(&args_of(&["--key", &path.to_string_lossy()]), &mut ok_stdin).unwrap();
        assert_eq!(unlocked, pubkey_hex);

        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(
            user_key_unlock(
                &args_of(&["--key", &path.to_string_lossy()]),
                &mut bad_stdin
            )
            .is_err()
        );
    }

    #[test]
    fn reveal_returns_same_words_for_v2_and_legacy() {
        let dir = tempfile::tempdir().unwrap();

        // v2: reveal requires the password.
        let v2_path = dir.path().join("v2.key");
        let mut init_stdin = stdin_of(&["a password"]);
        let (words, _) = user_key_init(
            &args_of(&["--out", &v2_path.to_string_lossy()]),
            &mut init_stdin,
        )
        .unwrap();
        let mut reveal_stdin = stdin_of(&["a password"]);
        let revealed = user_key_reveal(
            &args_of(&["--key", &v2_path.to_string_lossy()]),
            &mut reveal_stdin,
        )
        .unwrap();
        assert_eq!(revealed, words);

        // legacy: reveal tolerates an absent password line entirely.
        let legacy_path = dir.path().join("legacy.key");
        let seed = [42u8; 32];
        write_legacy(&legacy_path, &seed);
        let legacy_words = userkey::mnemonic_of_seed(&seed);

        let mut stdin = empty_stdin();
        let revealed_legacy = user_key_reveal(
            &args_of(&["--key", &legacy_path.to_string_lossy()]),
            &mut stdin,
        )
        .unwrap();
        assert_eq!(revealed_legacy, legacy_words);
    }

    #[test]
    fn encrypt_migrates_legacy_to_v2_preserving_pubkey_and_mnemonic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("user.key");
        let seed = [9u8; 32];
        write_legacy(&path, &seed);
        let expected_pubkey = pubkey_of(&seed);

        let mut before_stdin = empty_stdin();
        let words_before = user_key_reveal(
            &args_of(&["--key", &path.to_string_lossy()]),
            &mut before_stdin,
        )
        .unwrap();

        let mut encrypt_stdin = stdin_of(&["fresh password"]);
        let pubkey_after = user_key_encrypt(
            &args_of(&["--key", &path.to_string_lossy()]),
            &mut encrypt_stdin,
        )
        .unwrap();
        assert_eq!(pubkey_after, expected_pubkey);

        // already-v2 encrypt is a hard error, not a silent no-op.
        let mut second_stdin = stdin_of(&["another password"]);
        assert!(
            user_key_encrypt(
                &args_of(&["--key", &path.to_string_lossy()]),
                &mut second_stdin
            )
            .is_err()
        );

        let mut after_stdin = stdin_of(&["fresh password"]);
        let words_after = user_key_reveal(
            &args_of(&["--key", &path.to_string_lossy()]),
            &mut after_stdin,
        )
        .unwrap();
        assert_eq!(words_after, words_before);
    }

    #[test]
    fn status_reports_all_three_shapes() {
        let dir = tempfile::tempdir().unwrap();

        let absent_path = dir.path().join("absent.key");
        assert_eq!(
            user_key_status(&args_of(&["--key", &absent_path.to_string_lossy()])).unwrap(),
            "absent"
        );

        let plaintext_path = dir.path().join("plaintext.key");
        let seed = [3u8; 32];
        write_legacy(&plaintext_path, &seed);
        assert_eq!(
            user_key_status(&args_of(&["--key", &plaintext_path.to_string_lossy()])).unwrap(),
            format!("plaintext {}", pubkey_of(&seed))
        );

        let encrypted_path = dir.path().join("encrypted.key");
        let mut stdin = stdin_of(&["a password"]);
        let (_, init_pubkey_hex) = user_key_init(
            &args_of(&["--out", &encrypted_path.to_string_lossy()]),
            &mut stdin,
        )
        .unwrap();
        assert_eq!(
            user_key_status(&args_of(&["--key", &encrypted_path.to_string_lossy()])).unwrap(),
            format!("encrypted {init_pubkey_hex}")
        );
    }

    #[test]
    fn short_password_rejected_in_init_restore_encrypt() {
        let dir = tempfile::tempdir().unwrap();

        let init_path = dir.path().join("init.key");
        let mut stdin = stdin_of(&["short1"]);
        assert!(
            user_key_init(
                &args_of(&["--out", &init_path.to_string_lossy()]),
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
                &args_of(&["--out", &restore_path.to_string_lossy()]),
                &mut stdin
            )
            .is_err()
        );
        assert!(!restore_path.exists());

        let legacy_path = dir.path().join("legacy.key");
        let seed = [5u8; 32];
        write_legacy(&legacy_path, &seed);
        let mut stdin = stdin_of(&["short1"]);
        assert!(
            user_key_encrypt(
                &args_of(&["--key", &legacy_path.to_string_lossy()]),
                &mut stdin
            )
            .is_err()
        );
        // a rejected password must not have migrated the file.
        match userkey::read_user_key_file(&legacy_path).unwrap() {
            userkey::UserKeyFile::Plaintext(_) => {}
            userkey::UserKeyFile::Encrypted(_) => panic!("still-plaintext expected"),
        }
    }

    /// same seed, two custody shapes (legacy plaintext vs v2+password) must
    /// mint byte-identical bind JSON (ed25519 signing is deterministic), and
    /// that JSON must decode via `identity::decode_msg`.
    #[test]
    fn sign_bind_v2_password_matches_legacy_and_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [77u8; 32];
        // an arbitrary but VALID ed25519 point — a public key derived from a
        // seed, not raw bytes (not every 32-byte string is on-curve).
        let node_pub_hex = pubkey_of(&[100u8; 32]);

        let legacy_path = dir.path().join("legacy.key");
        write_legacy(&legacy_path, &seed);
        let mut stdin = empty_stdin();
        let legacy_json = user_sign_bind(
            &args_of(&[
                "--key",
                &legacy_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "0",
            ]),
            &mut stdin,
        )
        .unwrap();

        let v2_path = dir.path().join("v2.key");
        let line = userkey::seal_user_key(&seed, "a password").unwrap();
        userkey::write_user_key_new(&v2_path, &line).unwrap();
        let mut stdin = stdin_of(&["a password"]);
        let v2_json = user_sign_bind(
            &args_of(&[
                "--key",
                &v2_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "0",
            ]),
            &mut stdin,
        )
        .unwrap();

        assert_eq!(legacy_json, v2_json);

        match identity::decode_msg(legacy_json.as_bytes()).unwrap() {
            identity::IdentityMsg::BindNode { authorizer } => {
                assert_eq!(authorizer.key, pubkey_bytes(&seed));
                assert_eq!(authorizer.kind, identity::KeyKind::Ed25519);
            }
            other => panic!("expected BindNode, got {other:?}"),
        }

        // wrong password fails cleanly (and never silently falls back to
        // auto-generating a fresh legacy key underneath the v2 file).
        let mut bad_stdin = stdin_of(&["wrong password"]);
        assert!(
            user_sign_bind(
                &args_of(&[
                    "--key",
                    &v2_path.to_string_lossy(),
                    "--chain-id",
                    "test-chain",
                    "--node-pub",
                    &node_pub_hex,
                    "--nonce",
                    "0",
                ]),
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
        write_legacy(&key_path, &seed);
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
        let mut stdin = empty_stdin();
        let json = user_sign_gateway_route(
            &args_of(&[
                "--key",
                &key_path.to_string_lossy(),
                "--statement",
                &serde_json::to_string(&statement).unwrap(),
            ]),
            &mut stdin,
        )
        .unwrap();
        let gateway::GatewayMsg::SetRoute {
            statement: decoded,
            authorization,
        } = gateway::decode_msg(json.as_bytes()).unwrap();
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
        let mut stdin = empty_stdin();
        assert!(
            user_sign_gateway_route(
                &args_of(&[
                    "--key",
                    &key_path.to_string_lossy(),
                    "--statement",
                    &serde_json::to_string(&unsafe_statement).unwrap(),
                ]),
                &mut stdin,
            )
            .is_err()
        );
    }

    #[test]
    fn sign_unbind_v2_password_matches_legacy_and_decodes() {
        let dir = tempfile::tempdir().unwrap();
        let seed = [88u8; 32];
        let node_pub_hex = pubkey_of(&[101u8; 32]);

        let legacy_path = dir.path().join("legacy.key");
        write_legacy(&legacy_path, &seed);
        let mut stdin = empty_stdin();
        let legacy_json = user_sign_unbind(
            &args_of(&[
                "--key",
                &legacy_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "1",
            ]),
            &mut stdin,
        )
        .unwrap();

        let v2_path = dir.path().join("v2.key");
        let line = userkey::seal_user_key(&seed, "a password").unwrap();
        userkey::write_user_key_new(&v2_path, &line).unwrap();
        let mut stdin = stdin_of(&["a password"]);
        let v2_json = user_sign_unbind(
            &args_of(&[
                "--key",
                &v2_path.to_string_lossy(),
                "--chain-id",
                "test-chain",
                "--node-pub",
                &node_pub_hex,
                "--nonce",
                "1",
            ]),
            &mut stdin,
        )
        .unwrap();

        assert_eq!(legacy_json, v2_json);
        assert!(identity::decode_msg(legacy_json.as_bytes()).is_ok());
    }

    fn pubkey_bytes(seed: &[u8; 32]) -> Vec<u8> {
        ed25519::PrivateKey::decode(seed.as_slice())
            .unwrap()
            .public_key()
            .as_ref()
            .to_vec()
    }

    #[test]
    fn legacy_bare_generate_verb_still_works() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("legacy.key");
        cmd_user_key_generate_legacy(&args_of(&["--out", &path.to_string_lossy()])).unwrap();
        match userkey::read_user_key_file(&path).unwrap() {
            userkey::UserKeyFile::Plaintext(_) => {}
            userkey::UserKeyFile::Encrypted(_) => panic!("legacy generate must write v1"),
        }
    }

    #[test]
    fn unknown_subcommand_errors_cleanly() {
        let mut stdin = empty_stdin();
        let err = cmd_user_key(&args_of(&["bogus"]), &mut stdin).unwrap_err();
        assert!(err.to_string().contains("unknown user-key subcommand"));
    }

    #[test]
    fn sign_frame_round_trips_through_decode_frame() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        write_legacy(&key_path, &[7u8; 32]);

        // plaintext key: no password line — stdin is just the payload hex.
        let payload: &[u8] = b"\x00raw chunk bytes";
        let mut stdin = stdin_of(&[&hex_bytes(payload)]);
        let frame_hex = user_sign_frame(
            &args_of(&[
                "--key",
                key_path.to_str().unwrap(),
                "--target",
                "files",
                "--seq",
                "42",
            ]),
            &mut stdin,
        )
        .unwrap();

        let (origin, msg) =
            node::decode_frame(&config::unhex(&frame_hex).unwrap()).expect("frame verifies");
        let signer = ed25519::PrivateKey::decode([7u8; 32].as_slice()).unwrap();
        assert_eq!(origin, sdk::Origin::External(signer.public_key().as_ref().to_vec()));
        assert_eq!(msg.target, "files");
        assert_eq!(msg.payload, payload);
    }

    #[test]
    fn sign_admin_returns_owner_pop_the_verifier_would_accept() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        write_legacy(&key_path, &[7u8; 32]);

        // plaintext key ⇒ no password line ⇒ empty stdin (no payload either).
        let out = user_sign_admin(
            &args_of(&[
                "--key",
                key_path.to_str().unwrap(),
                "--method",
                "POST",
                "--path",
                "/v1/admin/shutdown",
            ]),
            &mut empty_stdin(),
        )
        .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&out).expect("one json line");
        let signer = ed25519::PrivateKey::decode([7u8; 32].as_slice()).unwrap();
        // the reported key is the user's own account key.
        assert_eq!(parsed["key"], hex_bytes(signer.public_key().as_ref()));
        // the signature is exactly what the node's verifier reconstructs over
        // the SAME (method, path, ts) — proving the verb signed the right bytes
        // with the right key (single source of truth: noded::admin::sign_admin).
        let ts: u64 = parsed["ts"].as_str().unwrap().parse().unwrap();
        let expect = noded::admin::sign_admin(&signer, "POST", "/v1/admin/shutdown", ts);
        assert_eq!(parsed["sig"], hex_bytes(expect.as_ref()));
    }

    #[test]
    fn sign_frame_reads_the_password_first_for_encrypted_keys() {
        let dir = tempfile::tempdir().unwrap();
        let key_path = dir.path().join("user.key");
        let line = userkey::seal_user_key(&[9u8; 32], "hunter2duck").unwrap();
        userkey::write_user_key_new(&key_path, &line).unwrap();

        let mut stdin = stdin_of(&["hunter2duck", &hex_bytes(b"{}")]);
        let frame_hex = user_sign_frame(
            &args_of(&[
                "--key",
                key_path.to_str().unwrap(),
                "--target",
                "files",
                "--seq",
                "0",
            ]),
            &mut stdin,
        )
        .unwrap();
        assert!(node::decode_frame(&config::unhex(&frame_hex).unwrap()).is_ok());

        // wrong password: refused before any payload is read.
        let mut stdin = stdin_of(&["wrong password", &hex_bytes(b"{}")]);
        assert!(
            user_sign_frame(
                &args_of(&[
                    "--key",
                    key_path.to_str().unwrap(),
                    "--target",
                    "files",
                    "--seq",
                    "0",
                ]),
                &mut stdin,
            )
            .is_err()
        );
    }
}
