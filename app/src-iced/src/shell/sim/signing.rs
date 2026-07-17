//! In-process frame signing for the sim lane: answers the two verbs the chat
//! write path uses (`user-key status`, `user-sign-frame`) with
//! `node::encode_frame` over one generated ed25519 key — no `user.key` file, no
//! subprocess. Installed once per process; the key is shared by every test in
//! the binary (like a fixture account) and derives from a fixed seed so
//! authorship is stable across runs.

use std::sync::OnceLock;

use commonware_cryptography::Signer as _;
use commonware_cryptography::ed25519::PrivateKey;

/// Fixed RNG seed for the fixture signer — stable authorship across runs.
const SIM_SIGNER_SEED: u64 = 0xD0CC;

fn signer() -> &'static PrivateKey {
    static KEY: OnceLock<PrivateKey> = OnceLock::new();
    KEY.get_or_init(|| PrivateKey::from_seed(SIM_SIGNER_SEED))
}

pub(super) fn author_pubkey_hex() -> String {
    hex_encode(signer().public_key().as_ref())
}

/// Route `Backend`'s signing verbs in-process. Idempotent and process-global:
/// after this, `sign_content_frame` and `signing_secrets` need no subprocess.
pub(super) fn install() {
    crate::backend::install_verb_override(Box::new(handle));
}

/// The override body, factored out so a unit test can drive the exact logic the
/// choke-point runs. An installed override is authoritative — an unrecognized
/// verb is an error, never a fall-through to `ducktape-node`.
fn handle(args: &[&str], stdin: &[&str]) -> Result<String, String> {
    match args {
        // signing_secrets() + identity_state() run: user-key status --key <path>.
        // `parse_key_status` reads the last line; "plaintext <pubkey>" yields a
        // non-Locked state, so `signing_secrets` returns no secret stdin.
        ["user-key", "status", ..] => Ok(format!("plaintext {}", author_pubkey_hex())),
        // sign_frame() runs: user-sign-frame --key <p> --target <t> --seq <n>,
        // with the payload hex as the LAST stdin line (secrets are empty here).
        ["user-sign-frame", rest @ ..] => {
            let target = flag_value(rest, "--target")?;
            let seq: u64 = flag_value(rest, "--seq")?
                .parse()
                .map_err(|error| format!("user-sign-frame --seq: {error}"))?;
            let payload_hex = stdin
                .last()
                .ok_or("user-sign-frame: missing payload line")?;
            let payload = hex_decode(payload_hex)?;
            let frame = node::encode_frame(
                signer(),
                seq,
                &sdk::Msg {
                    target: target.to_string(),
                    payload,
                },
            );
            Ok(hex_encode(&frame))
        }
        other => Err(format!(
            "sim verb override: unhandled verb {:?}",
            other.first()
        )),
    }
}

fn flag_value<'a>(args: &[&'a str], flag: &str) -> Result<&'a str, String> {
    args.iter()
        .position(|arg| *arg == flag)
        .and_then(|index| args.get(index + 1))
        .copied()
        .ok_or_else(|| format!("user-sign-frame: missing {flag}"))
}

fn hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").expect("write to String is infallible");
    }
    out
}

fn hex_decode(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("payload hex has an odd length".to_string());
    }
    (0..hex.len())
        .step_by(2)
        .map(|start| {
            u8::from_str_radix(&hex[start..start + 2], 16)
                .map_err(|error| format!("payload hex: {error}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The override produces a frame the kernel verifies: encode via the
    /// override body (as `Backend` would over the choke-point), then decode via
    /// `node::decode_frame`, which VERIFIES the signature and returns the
    /// authenticated `(origin, msg)`.
    #[test]
    fn override_signs_verifiable_frames() {
        // Prove the installer wires without panic and is idempotent.
        install();
        install();

        // status must parse as a non-Locked (plaintext) key so signing_secrets
        // returns no secret stdin.
        let status = handle(&["user-key", "status", "--key", "/dev/null"], &[]).expect("status");
        assert_eq!(status, format!("plaintext {}", author_pubkey_hex()));

        // Reproduce the exact argv + stdin sign_frame() sends for a plaintext
        // key: no secret lines, payload hex last.
        let payload = serde_json::to_vec(&serde_json::json!({ "noop": {} })).expect("payload");
        let payload_hex = hex_encode(&payload);
        let out = handle(
            &[
                "user-sign-frame",
                "--key",
                "/dev/null",
                "--target",
                "chat",
                "--seq",
                "7",
            ],
            &[&payload_hex],
        )
        .expect("sign frame");

        let frame = hex_decode(&out).expect("frame hex");
        let (origin, msg) = node::decode_frame(&frame).expect("frame verifies");
        assert_eq!(msg.target, "chat");
        assert_eq!(msg.payload, payload);
        assert_eq!(
            origin,
            sdk::Origin::External(signer().public_key().as_ref().to_vec()),
            "authorship rides the fixture signer's key",
        );
    }

    #[test]
    fn unhandled_verbs_error_without_subprocess() {
        assert!(handle(&["user-key", "init"], &[]).is_err());
        assert!(handle(&["join"], &[]).is_err());
    }
}
