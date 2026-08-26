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
