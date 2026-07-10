//! Coordinator policy selection — the ONLY new decision the untrusted
//! coordinator makes at boot: which [`nat_traversal::AuthPolicy`] to serve.
//!
//! Factored out of `main.rs` so it is unit-testable without spawning the
//! process. The coordinator stays keyless: `--genesis-set` reads ONLY the
//! PUBLIC validator pubkeys out of a `network.toml` (never a secret, never
//! written back), and every other input is a bare CLI flag.

use commonware_codec::DecodeExt as _;
use commonware_cryptography::ed25519;
use serde::Deserialize;

/// Linux process CPU time across every thread, in nanoseconds.
#[cfg(target_os = "linux")]
pub fn process_cpu_ns() -> Option<u64> {
    let mut total = 0u64;
    let mut found = false;
    for entry in std::fs::read_dir("/proc/self/task").ok()?.flatten() {
        let Ok(text) = std::fs::read_to_string(entry.path().join("schedstat")) else {
            continue;
        };
        let Some(runtime) = text
            .split_whitespace()
            .next()
            .and_then(|raw| raw.parse::<u64>().ok())
        else {
            continue;
        };
        total = total.saturating_add(runtime);
        found = true;
    }
    found.then_some(total)
}

#[cfg(not(target_os = "linux"))]
pub fn process_cpu_ns() -> Option<u64> {
    None
}

#[cfg(target_os = "linux")]
pub fn process_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kib = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .split_whitespace()
        .next()?
        .parse::<u64>()
        .ok()?;
    kib.checked_mul(1024)
}

#[cfg(not(target_os = "linux"))]
pub fn process_rss_bytes() -> Option<u64> {
    None
}

/// The one field of `network.toml` the coordinator cares about: the genesis
/// validators, as hex ed25519 public keys. Every other key (chain_id, scheme,
/// bootstrap, reach, coordination, …) is ignored — serde drops unknown fields —
/// so a full descriptor parses here without dragging in `bin/node`.
#[derive(Debug, Deserialize)]
struct GenesisPin {
    #[serde(default)]
    validators: Vec<String>,
}

/// Select the authorization policy from CLI flags:
/// `--genesis-set <path>` => Private (pinned to that network.toml's valset);
/// `--allow-anonymous`    => fully-open (legacy);
/// otherwise              => public with proof-of-possession (deployed default).
pub fn select_policy(args: &[String]) -> std::io::Result<nat_traversal::AuthPolicy> {
    let allow_anonymous = args.iter().any(|a| a == "--allow-anonymous");
    let has_genesis_set = args.iter().any(|a| a == "--genesis-set");
    // `--allow-anonymous` and `--genesis-set` are mutually exclusive (the USAGE
    // string declares them so with `|`). Passing BOTH is contradictory, so it is
    // a HARD error and NOT a silent pick of the weaker policy. Failing closed
    // here is what keeps a stray or env-templated `--allow-anonymous` from
    // quietly disabling a genesis pin — the same "malformed/conflicting args
    // hard-fail, never downgrade to a weaker policy" contract the value-less
    // `--genesis-set` check below already upholds. (Previously `--allow-anonymous`
    // short-circuited first and silently won, downgrading a Private coordinator
    // to fully-open on a config mistake.)
    if allow_anonymous && has_genesis_set {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "--allow-anonymous and --genesis-set are mutually exclusive",
        ));
    }
    if allow_anonymous {
        return Ok(nat_traversal::AuthPolicy::Open { require_pop: false });
    }
    // `--genesis-set` presence is detected SEPARATELY from its value: a present
    // but value-less flag (bare `--genesis-set`, `--genesis-set` as the final
    // token, or immediately followed by another `--flag` — e.g. an unset shell
    // variable that collapses to nothing) is a HARD error, never a silent
    // fall-through to the weaker `Open { require_pop: true }`. Downgrading a
    // Private (genesis/cap-gated) coordinator to public-PoP on a typo'd path
    // would admit any node with a valid proof-of-possession.
    if let Some(i) = args.iter().position(|a| a == "--genesis-set") {
        let path = args
            .get(i + 1)
            .filter(|v| !v.starts_with("--"))
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "--genesis-set requires a <network.toml> path",
                )
            })?;
        let genesis_set = load_genesis_pubkeys(path)?;
        return Ok(nat_traversal::AuthPolicy::Private { genesis_set });
    }
    Ok(nat_traversal::AuthPolicy::Open { require_pop: true })
}

/// Parse the PUBLIC genesis validator pubkeys out of a `network.toml`. This is
/// the ONLY new input the coordinator reads — public data, never a secret.
/// Mirrors `NetworkDescriptor::validator_keys` (bin/node/src/config.rs) without
/// depending on the node crate: decode each hex entry to an ed25519 pubkey and
/// reject a duplicate (a repeat would otherwise be a silently smaller valset).
fn load_genesis_pubkeys(path: &str) -> std::io::Result<Vec<ed25519::PublicKey>> {
    let text = std::fs::read_to_string(path)?;
    let pin: GenesisPin = toml::from_str(&text).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("network.toml: {e}"),
        )
    })?;
    let invalid = |msg: String| std::io::Error::new(std::io::ErrorKind::InvalidData, msg);

    let keys: Vec<ed25519::PublicKey> = pin
        .validators
        .iter()
        .map(|h| decode_key(h))
        .collect::<Result<_, _>>()
        .map_err(invalid)?;

    let mut seen = std::collections::BTreeSet::new();
    for k in &keys {
        if !seen.insert(k.as_ref().to_vec()) {
            return Err(invalid(format!(
                "duplicate validator {} in genesis set",
                hex_bytes(k.as_ref())
            )));
        }
    }
    Ok(keys)
}

/// Decode one hex-encoded ed25519 public key. Dependency-free hex (the
/// coordinator does not pull in bin/node's `unhex`); strict digits, even length.
fn decode_key(hex: &str) -> Result<ed25519::PublicKey, String> {
    let raw = unhex(hex.trim())?;
    ed25519::PublicKey::decode(raw.as_slice())
        .map_err(|e| format!("{hex:?} is not an ed25519 public key: {e}"))
}

fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("hex string contains non-hex characters".into());
    }
    if !s.len().is_multiple_of(2) {
        return Err("hex string has odd length".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|e| e.to_string()))
        .collect()
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
