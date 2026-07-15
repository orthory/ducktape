//! User-key custody and native Touch ID for the iced shell.

#[cfg(test)]
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use zeroize::Zeroize as _;

use super::Backend;
use super::node_control::{last_line, run_verb, run_verb_with_stdin};
use super::private_fs;

const MIN_PASSWORD_CHARS: usize = 8;
const MAX_PASSWORD_BYTES: usize = 16 * 1024;
const MNEMONIC_WORDS: usize = 24;
const MAX_MNEMONIC_BYTES: usize = 4 * 1024;

/// The identity gate's four durable/session states.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum IdentityStatus {
    Absent,
    Plaintext,
    Locked,
    Unlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityState {
    pub state: IdentityStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubkey: Option<String>,
    pub mnemonic_confirmed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityPubkey {
    pub pubkey: String,
}

/// A recovery phrase that scrubs its allocation on drop and never prints via
/// `Debug` or `Display`.
#[derive(Clone)]
pub struct RecoveryPhrase(SecretString);

impl RecoveryPhrase {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn words(&self) -> impl Iterator<Item = &str> {
        self.0.split_whitespace()
    }
}

impl std::fmt::Debug for RecoveryPhrase {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("RecoveryPhrase([REDACTED])")
    }
}

#[derive(Clone)]
pub struct IdentityCreated {
    pub pubkey: String,
    pub mnemonic: RecoveryPhrase,
}

impl std::fmt::Debug for IdentityCreated {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityCreated")
            .field("pubkey", &self.pubkey)
            .field("mnemonic", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone)]
pub struct IdentityMnemonic {
    pub mnemonic: RecoveryPhrase,
}

impl std::fmt::Debug for IdentityMnemonic {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("IdentityMnemonic")
            .field("mnemonic", &"[REDACTED]")
            .finish()
    }
}

pub(super) struct SecretString(String);

impl SecretString {
    pub(super) fn new(value: String) -> Self {
        Self(value)
    }
}

impl Clone for SecretString {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl std::ops::Deref for SecretString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for SecretString {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Drop for SecretString {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

struct SecretBytes(Vec<u8>);

impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

static SESSION_PASSWORD: Mutex<Option<SecretString>> = Mutex::new(None);

fn session_lock() -> std::sync::MutexGuard<'static, Option<SecretString>> {
    SESSION_PASSWORD
        .lock()
        .unwrap_or_else(|error| error.into_inner())
}

fn cache_store(password: &str) {
    *session_lock() = Some(SecretString::new(password.to_string()));
}

fn cache_peek() -> Option<SecretString> {
    session_lock().clone()
}

fn cache_clear() {
    session_lock().take();
}

impl Backend {
    pub async fn identity_state(&self) -> Result<IdentityState, String> {
        let root = self.root.clone();
        self.control
            .run(move || identity_state_blocking(&root))
            .await
    }

    pub async fn create_identity(&self, password: String) -> Result<IdentityCreated, String> {
        let root = self.root.clone();
        let password = SecretString::new(password);
        self.control
            .run(move || create_identity_blocking(&root, password))
            .await
    }

    /// Create an account using a random password that never crosses the UI.
    /// Call [`Backend::touch_id_enroll_session`] after recovery confirmation.
    pub async fn create_identity_for_touch_id(&self) -> Result<IdentityCreated, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let password = random_vault_password()?;
                create_identity_blocking(&root, password)
            })
            .await
    }

    pub async fn restore_identity(
        &self,
        mnemonic: String,
        password: String,
    ) -> Result<IdentityPubkey, String> {
        let root = self.root.clone();
        let mnemonic = normalize_mnemonic(SecretString::new(mnemonic))?;
        let password = SecretString::new(password);
        self.control
            .run(move || restore_identity_blocking(&root, mnemonic, password))
            .await
    }

    pub async fn unlock_identity(&self, password: String) -> Result<IdentityPubkey, String> {
        let root = self.root.clone();
        let password = SecretString::new(password);
        self.control
            .run(move || unlock_with_secret(&root, password))
            .await
    }

    pub async fn reveal_identity(&self, password: String) -> Result<IdentityMnemonic, String> {
        let root = self.root.clone();
        let password = SecretString::new(password);
        self.control
            .run(move || reveal_identity_blocking(&root, password))
            .await
    }

    pub async fn encrypt_legacy_identity(
        &self,
        password: String,
    ) -> Result<IdentityPubkey, String> {
        let root = self.root.clone();
        let password = SecretString::new(password);
        self.control
            .run(move || encrypt_legacy_blocking(&root, password))
            .await
    }

    /// Persist the UX-only recovery confirmation bit. Word verification stays
    /// in the iced confirmation screen, matching the previous desktop flow.
    pub async fn confirm_recovery(&self) -> Result<(), String> {
        let root = self.root.clone();
        self.control
            .run(move || set_mnemonic_confirmed(&root))
            .await
    }

    pub async fn lock_identity(&self) -> Result<(), String> {
        self.control
            .run(|| {
                cache_clear();
                Ok(())
            })
            .await
    }

    pub async fn touch_id_available(&self) -> Result<bool, String> {
        self.control.run(|| Ok(touch_id::available())).await
    }

    pub async fn touch_id_enrolled(&self) -> Result<bool, String> {
        self.control.run(|| Ok(touch_id::enrolled())).await
    }

    pub async fn touch_id_enroll(&self, passphrase: String) -> Result<(), String> {
        let root = self.root.clone();
        let passphrase = SecretString::new(passphrase);
        validate_enrollment_secret(&passphrase)?;
        self.control
            .run(move || {
                unlock_with_secret(&root, passphrase.clone())?;
                touch_id::enroll(passphrase)
            })
            .await
    }

    /// Enroll the already-verified session password. This is the safe second
    /// half of [`Backend::create_identity_for_touch_id`].
    pub async fn touch_id_enroll_session(&self) -> Result<(), String> {
        self.control
            .run(|| {
                let password = cache_peek().ok_or_else(|| "identity-locked".to_string())?;
                validate_enrollment_secret(&password)?;
                touch_id::enroll(password)
            })
            .await
    }

    pub async fn touch_id_unlock(&self) -> Result<IdentityPubkey, String> {
        let root = self.root.clone();
        self.control
            .run(move || {
                let password = touch_id::read_passphrase()?;
                let pubkey = unlock_with_secret(&root, password.clone())?;
                let _ = touch_id::enroll(password);
                Ok(pubkey)
            })
            .await
    }

    pub async fn touch_id_disable(&self) -> Result<(), String> {
        self.control.run(touch_id::disable).await
    }
}

fn user_key_path(root: &Path) -> PathBuf {
    root.join("user.key")
}

/// Secret stdin for a signing verb: empty for absent/plaintext keys, or the
/// verified session password for an encrypted key.
pub(super) fn signing_secrets(root: &Path) -> Result<Vec<SecretString>, String> {
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = run_verb(&["user-key", "status", "--key", &key])?;
    let (state, _) = parse_key_status(&last_line(&stdout))?;
    if state != IdentityStatus::Locked {
        return Ok(Vec::new());
    }
    cache_peek()
        .map(|password| vec![password])
        .ok_or_else(|| "identity-locked".to_string())
}

fn validate_secret_line(secret: &str, field: &str, max_bytes: usize) -> Result<(), String> {
    if secret.len() > max_bytes {
        return Err(format!("{field} is too long"));
    }
    if secret
        .as_bytes()
        .iter()
        .any(|byte| matches!(byte, 0 | b'\n' | b'\r'))
    {
        return Err(format!("{field} contains an unsupported line delimiter"));
    }
    Ok(())
}

fn validate_new_password(password: &str) -> Result<(), String> {
    validate_secret_line(password, "password", MAX_PASSWORD_BYTES)?;
    if password.trim().is_empty() || password.chars().count() < MIN_PASSWORD_CHARS {
        return Err(format!(
            "password must be at least {MIN_PASSWORD_CHARS} characters"
        ));
    }
    Ok(())
}

fn validate_existing_password(password: &str) -> Result<(), String> {
    validate_secret_line(password, "password", MAX_PASSWORD_BYTES)
}

fn validate_enrollment_secret(password: &str) -> Result<(), String> {
    validate_existing_password(password)?;
    if password.is_empty() {
        return Err("password is required".to_string());
    }
    Ok(())
}

fn normalize_mnemonic(mnemonic: SecretString) -> Result<SecretString, String> {
    if mnemonic.len() > MAX_MNEMONIC_BYTES {
        return Err("recovery phrase is too long".to_string());
    }
    let words: Vec<&str> = mnemonic.split_whitespace().collect();
    if words.len() != MNEMONIC_WORDS {
        return Err(format!(
            "recovery phrase must contain exactly {MNEMONIC_WORDS} words"
        ));
    }
    if words
        .iter()
        .any(|word| word.is_empty() || word.contains('\0'))
    {
        return Err("recovery phrase contains an invalid word".to_string());
    }
    Ok(SecretString::new(words.join(" ")))
}

fn parse_pubkey(line: &str, context: &str) -> Result<String, String> {
    let pubkey = line.trim();
    if pubkey.is_empty() || pubkey.len() > 1024 || pubkey.chars().any(char::is_whitespace) {
        return Err(format!("{context} returned a malformed public key"));
    }
    Ok(pubkey.to_string())
}

fn parse_key_status(line: &str) -> Result<(IdentityStatus, Option<String>), String> {
    let line = line.trim();
    if line == "absent" {
        return Ok((IdentityStatus::Absent, None));
    }
    if let Some(pubkey) = line.strip_prefix("plaintext ") {
        return Ok((
            IdentityStatus::Plaintext,
            Some(parse_pubkey(pubkey, "user-key status")?),
        ));
    }
    if let Some(pubkey) = line.strip_prefix("encrypted ") {
        return Ok((
            IdentityStatus::Locked,
            Some(parse_pubkey(pubkey, "user-key status")?),
        ));
    }
    Err(format!(
        "unrecognized user-key status output ({} chars)",
        line.chars().count()
    ))
}

fn value_lines(stdout: &str) -> Vec<&str> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect()
}

fn parse_init_output(stdout: &str) -> Result<IdentityCreated, String> {
    let lines = value_lines(stdout);
    match lines.as_slice() {
        [mnemonic, pubkey] => Ok(IdentityCreated {
            pubkey: parse_pubkey(pubkey, "user-key init")?,
            mnemonic: RecoveryPhrase(normalize_mnemonic(SecretString::new(mnemonic.to_string()))?),
        }),
        other => Err(format!(
            "user-key init: expected mnemonic + pubkey lines, got {} line(s)",
            other.len()
        )),
    }
}

fn identity_state_blocking(root: &Path) -> Result<IdentityState, String> {
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = run_verb(&["user-key", "status", "--key", &key])?;
    let (raw_state, pubkey) = parse_key_status(&last_line(&stdout))?;
    let state = if raw_state == IdentityStatus::Locked && cache_peek().is_some() {
        IdentityStatus::Unlocked
    } else {
        raw_state
    };
    Ok(IdentityState {
        state,
        pubkey,
        mnemonic_confirmed: mnemonic_confirmed(root)?,
    })
}

fn create_identity_blocking(
    root: &Path,
    password: SecretString,
) -> Result<IdentityCreated, String> {
    validate_new_password(&password)?;
    private_fs::ensure_private_dir(root)?;
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = SecretString::new(run_verb_with_stdin(
        &["user-key", "init", "--out", &key],
        &[&password],
    )?);
    let created = parse_init_output(&stdout)?;
    cache_store(&password);
    Ok(created)
}

fn restore_identity_blocking(
    root: &Path,
    mnemonic: SecretString,
    password: SecretString,
) -> Result<IdentityPubkey, String> {
    validate_new_password(&password)?;
    private_fs::ensure_private_dir(root)?;
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = run_verb_with_stdin(
        &["user-key", "restore", "--out", &key],
        &[&mnemonic, &password],
    )?;
    let pubkey = parse_pubkey(&last_line(&stdout), "user-key restore")?;
    cache_store(&password);
    set_mnemonic_confirmed(root)?;
    Ok(IdentityPubkey { pubkey })
}

fn unlock_with_secret(root: &Path, password: SecretString) -> Result<IdentityPubkey, String> {
    validate_existing_password(&password)?;
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = run_verb_with_stdin(&["user-key", "unlock", "--key", &key], &[&password])?;
    let pubkey = parse_pubkey(&last_line(&stdout), "user-key unlock")?;
    cache_store(&password);
    Ok(IdentityPubkey { pubkey })
}

fn reveal_identity_blocking(
    root: &Path,
    password: SecretString,
) -> Result<IdentityMnemonic, String> {
    validate_existing_password(&password)?;
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = SecretString::new(run_verb_with_stdin(
        &["user-key", "reveal", "--key", &key],
        &[&password],
    )?);
    let mnemonic = RecoveryPhrase(normalize_mnemonic(SecretString::new(last_line(&stdout)))?);
    cache_store(&password);
    Ok(IdentityMnemonic { mnemonic })
}

fn encrypt_legacy_blocking(root: &Path, password: SecretString) -> Result<IdentityPubkey, String> {
    validate_new_password(&password)?;
    let key = user_key_path(root).to_string_lossy().into_owned();
    let stdout = run_verb_with_stdin(&["user-key", "encrypt", "--key", &key], &[&password])?;
    let pubkey = parse_pubkey(&last_line(&stdout), "user-key encrypt")?;
    cache_store(&password);
    set_mnemonic_confirmed(root)?;
    Ok(IdentityPubkey { pubkey })
}

fn random_vault_password() -> Result<SecretString, String> {
    let mut bytes = SecretBytes(vec![0; 32]);
    getrandom::getrandom(&mut bytes.0)
        .map_err(|_| "could not generate a vault secret".to_string())?;
    let mut password = String::with_capacity(64);
    use std::fmt::Write as _;
    for byte in &bytes.0 {
        write!(password, "{byte:02x}").expect("write to String");
    }
    Ok(SecretString::new(password))
}

fn registry_path(root: &Path) -> PathBuf {
    root.join("registry.json")
}

fn registry_value(root: &Path) -> Result<serde_json::Value, String> {
    let _ = super::workspaces::snapshot_at(root)?;
    let path = registry_path(root);
    match private_fs::read_to_string(&path)? {
        Some(text) => serde_json::from_str(&text)
            .map_err(|_| "workspace registry could not be parsed".to_string()),
        None => Ok(serde_json::json!({
            "version": 1,
            "active": null,
            "workspaces": [],
            "mnemonicConfirmed": false
        })),
    }
}

fn mnemonic_confirmed(root: &Path) -> Result<bool, String> {
    let value = registry_value(root)?;
    match value.get("mnemonicConfirmed") {
        Some(value) => value
            .as_bool()
            .ok_or_else(|| "workspace registry has an invalid confirmation flag".to_string()),
        None => Ok(false),
    }
}

fn set_mnemonic_confirmed(root: &Path) -> Result<(), String> {
    let mut value = registry_value(root)?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| "workspace registry is not an object".to_string())?;
    if object
        .get("mnemonicConfirmed")
        .and_then(|value| value.as_bool())
        == Some(true)
    {
        return Ok(());
    }
    object.insert("mnemonicConfirmed".to_string(), true.into());
    private_fs::ensure_private_dir(root)?;
    let bytes = serde_json::to_vec_pretty(&value)
        .map_err(|_| "workspace registry could not be serialized".to_string())?;
    private_fs::write_atomic(&registry_path(root), &bytes)
}

#[cfg(not(target_os = "macos"))]
mod touch_id {
    use super::SecretString;

    pub(super) fn available() -> bool {
        false
    }

    pub(super) fn enrolled() -> bool {
        false
    }

    pub(super) fn enroll(_passphrase: SecretString) -> Result<(), String> {
        Err("Touch ID is only available on macOS".to_string())
    }

    pub(super) fn read_passphrase() -> Result<SecretString, String> {
        Err("Touch ID is only available on macOS".to_string())
    }

    pub(super) fn disable() -> Result<(), String> {
        Err("Touch ID is only available on macOS".to_string())
    }
}

#[cfg(target_os = "macos")]
mod touch_id {
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::item::{ItemClass, ItemSearchOptions};
    use security_framework::passwords::{
        PasswordOptions, delete_generic_password, get_generic_password, set_generic_password,
        set_generic_password_options,
    };

    use super::{SecretBytes, SecretString};

    const SERVICE: &str = "com.ducktape.app.userkey";
    const ACCOUNT: &str = "vault-passphrase";
    const PLAIN_ACCOUNT: &str = "vault-passphrase.plain";
    const USER_PRESENCE: usize = 1 << 0;
    const ERR_SEC_USER_CANCELED: i32 = -128;
    const ERR_SEC_MISSING_ENTITLEMENT: i32 = -34018;
    const LA_ERROR_USER_CANCEL: isize = -2;

    fn user_presence_access() -> Result<SecAccessControl, String> {
        SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            USER_PRESENCE,
        )
        .map_err(|error| format!("access-control: {error}"))
    }

    pub(super) fn available() -> bool {
        user_presence_access().is_ok()
    }

    pub(super) fn enrolled() -> bool {
        has_item(ACCOUNT) || has_item(PLAIN_ACCOUNT)
    }

    fn has_item(account: &str) -> bool {
        ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(SERVICE)
            .account(account)
            .search()
            .is_ok()
    }

    pub(super) fn enroll(passphrase: SecretString) -> Result<(), String> {
        let access = user_presence_access()?;
        let _ = disable();
        let mut options = PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
        options.set_access_control(access);
        match set_generic_password_options(passphrase.as_bytes(), options) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERR_SEC_MISSING_ENTITLEMENT => {
                set_generic_password(SERVICE, PLAIN_ACCOUNT, passphrase.as_bytes())
                    .map_err(|error| format!("keychain add (plain): {error}"))
            }
            Err(error) => Err(format!("keychain add: {error}")),
        }
    }

    pub(super) fn disable() -> Result<(), String> {
        for account in [ACCOUNT, PLAIN_ACCOUNT] {
            let _ = delete_generic_password(SERVICE, account);
        }
        Ok(())
    }

    pub(super) fn read_passphrase() -> Result<SecretString, String> {
        let bytes = match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => bytes,
            Err(error) if error.code() == ERR_SEC_USER_CANCELED => {
                return Err("touchid-canceled".to_string());
            }
            Err(_) if has_item(PLAIN_ACCOUNT) => {
                user_presence_check()?;
                get_generic_password(SERVICE, PLAIN_ACCOUNT)
                    .map_err(|_| "touchid-unavailable".to_string())?
            }
            Err(_) => return Err("touchid-unavailable".to_string()),
        };
        let bytes = SecretBytes(bytes);
        let passphrase = std::str::from_utf8(&bytes.0)
            .map_err(|_| "corrupt keychain item".to_string())?
            .to_string();
        Ok(SecretString::new(passphrase))
    }

    fn user_presence_check() -> Result<(), String> {
        use block2::RcBlock;
        use objc2_foundation::{NSError, NSString};
        use objc2_local_authentication::{LAContext, LAPolicy};

        let (sender, receiver) = std::sync::mpsc::channel();
        let context;
        unsafe {
            context = LAContext::new();
            let reason = NSString::from_str("unlock your Ducktape account");
            let reply = RcBlock::new(move |ok: objc2::runtime::Bool, error: *mut NSError| {
                let code = if error.is_null() { 0 } else { (*error).code() };
                let _ = sender.send((ok.as_bool(), code));
            });
            context.evaluatePolicy_localizedReason_reply(
                LAPolicy::DeviceOwnerAuthentication,
                &reason,
                &reply,
            );
        }
        let outcome = receiver.recv();
        drop(context);
        match outcome {
            Ok((true, _)) => Ok(()),
            Ok((false, LA_ERROR_USER_CANCEL)) => Err("touchid-canceled".to_string()),
            Ok((false, code)) => Err(format!("touchid-unavailable (LAError {code})")),
            Err(_) => Err("touchid-unavailable".to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words() -> String {
        (0..MNEMONIC_WORDS)
            .map(|index| format!("word{index}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn scratch(tag: &str) -> PathBuf {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "ducktape-iced-identity-{tag}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn password_policy_counts_characters_and_rejects_protocol_delimiters() {
        assert!(validate_new_password("1234567").is_err());
        assert!(validate_new_password("🔒🔒🔒🔒🔒🔒🔒🔒").is_ok());
        assert!(validate_new_password("        ").is_err());
        assert!(validate_new_password("valid-pass\nword").is_err());
        assert!(validate_existing_password("").is_ok());
        assert!(validate_enrollment_secret("").is_err());
    }

    #[test]
    fn mnemonic_is_normalized_and_requires_exact_word_count() {
        let spaced = format!("  {}\n", words().replace(' ', "  "));
        let normalized = normalize_mnemonic(SecretString::new(spaced)).unwrap();
        assert_eq!(normalized.split_whitespace().count(), MNEMONIC_WORDS);
        let error = match normalize_mnemonic(SecretString::new("secret words".to_string())) {
            Ok(_) => panic!("short recovery phrase was accepted"),
            Err(error) => error,
        };
        assert!(!error.contains("secret"));
    }

    #[test]
    fn parsers_never_echo_secret_output_in_errors() {
        let error = parse_key_status("garbled hunter2-secret").unwrap_err();
        assert!(!error.contains("hunter2"));
        let error = parse_init_output("abandon ability able").unwrap_err();
        assert!(!error.contains("abandon"));
    }

    #[test]
    fn init_output_returns_redacted_zeroizing_phrase() {
        let stdout = format!("{}\ndeadbeef\n", words());
        let created = parse_init_output(&stdout).unwrap();
        assert_eq!(created.mnemonic.words().count(), MNEMONIC_WORDS);
        assert!(!format!("{created:?}").contains("word0"));
    }

    #[test]
    fn session_cache_round_trips_replaces_and_clears() {
        cache_clear();
        assert!(cache_peek().is_none());
        cache_store("correct horse battery staple");
        assert_eq!(
            cache_peek().as_deref(),
            Some("correct horse battery staple")
        );
        cache_store("replacement password");
        assert_eq!(cache_peek().as_deref(), Some("replacement password"));
        cache_clear();
        assert!(cache_peek().is_none());
    }

    #[test]
    fn confirmation_round_trip_preserves_registry_fields() {
        let root = scratch("confirm");
        fs::write(
            registry_path(&root),
            r#"{"version":1,"active":null,"workspaces":[],"extra":"kept"}"#,
        )
        .unwrap();
        assert!(!mnemonic_confirmed(&root).unwrap());
        set_mnemonic_confirmed(&root).unwrap();
        assert!(mnemonic_confirmed(&root).unwrap());
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(registry_path(&root)).unwrap()).unwrap();
        assert_eq!(value["extra"], "kept");
        fs::remove_dir_all(root).ok();
    }

    #[test]
    fn random_vault_password_is_bounded_and_distinct() {
        let first = random_vault_password().unwrap();
        let second = random_vault_password().unwrap();
        assert_eq!(first.len(), 64);
        assert_ne!(first.as_ref(), second.as_ref());
        assert!(validate_new_password(&first).is_ok());
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn touch_id_is_explicitly_unavailable_off_macos() {
        assert!(!touch_id::available());
        assert!(!touch_id::enrolled());
        assert!(touch_id::enroll(SecretString::new("password".into())).is_err());
        assert!(touch_id::read_passphrase().is_err());
        assert!(touch_id::disable().is_err());
    }
}
