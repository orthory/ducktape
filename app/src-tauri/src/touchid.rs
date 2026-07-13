//! Native macOS Touch ID custody: the vault passphrase in the Keychain, with
//! user presence (Touch ID when the sensor is usable, the Mac's login
//! password when it isn't — lid closed, clamshell, changed fingerprint set)
//! demanded before it is released. Two-rung ladder, per item:
//!
//! 1. A user-presence-ACL item in the data-protection keychain — the OS
//!    itself prompts on read. Only possible in a build signed with an
//!    application-identifier entitlement; `SecItemAdd` returns
//!    errSecMissingEntitlement (-34018) otherwise, dev/ad-hoc builds always.
//! 2. Fallback: a plain login-keychain item under a distinct account name,
//!    with the same user-presence sheet demanded by us through `LAContext`
//!    (`deviceOwnerAuthentication` = biometry-or-password) before the read.
//!
//! No Secure-Enclave key generation, no chain member key — this only changes
//! how `user_identity_unlock`'s passphrase is supplied. Non-macOS targets
//! compile working stubs.
//!
//! macOS note: under the CEF runtime, biometric prompts need a real UI session.
//! For automated macOS QA the `--use-mock-keychain` Chromium flag is NOT used
//! here — these commands talk to the OS Keychain via `security-framework`, not
//! Chromium's keystore. The biometric-prompt path is manual-only (see tests).

// Read only by the macOS `imp`; the non-macOS stubs don't touch the Keychain.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const KEYCHAIN_SERVICE: &str = "com.ducktape.app.userkey";
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const KEYCHAIN_ACCOUNT: &str = "vault-passphrase";
// The entitlement-less fallback item (rung 2 — see the module header). The
// account name is the discriminator: its presence is what tells `unlock` to
// run the LAContext gate. That gate is app-enforced only: a debugger or
// another local process could read this item without it (subject to the
// login keychain's own per-app ACL prompt) — the deliberate tradeoff for
// unentitled builds, with the 24-word phrase as the custody backstop.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const KEYCHAIN_ACCOUNT_PLAIN: &str = "vault-passphrase.plain";

/// True only on macOS where user-presence auth (see the module header) works.
/// Gates every piece of Touch ID UI.
#[tauri::command]
pub async fn touchid_available() -> bool {
    imp::available()
}

/// Store the vault passphrase behind user-presence custody. Called once,
/// right after the recovery phrase is confirmed.
#[tauri::command]
pub async fn touchid_enroll(passphrase: String) -> Result<(), String> {
    imp::enroll(passphrase)
}

/// Non-prompting presence check: is a Touch ID passphrase item enrolled? Reads
/// item metadata only (`kSecReturnData=false`), so it never triggers a
/// biometric prompt. Drives the "Enabled · Disable" vs "Enable Touch ID" row.
#[tauri::command]
pub async fn touchid_enrolled() -> bool {
    imp::enrolled()
}

/// Retrieve the passphrase behind the user-presence prompt, unlock the vault,
/// cache it. Returns the pubkey, exactly like `user_identity_unlock`.
#[tauri::command]
pub async fn touchid_unlock(
    app: crate::rt::AppHandle,
    window: crate::rt::WebviewWindow,
    control: tauri::State<'_, crate::daemon::NodeControl>,
) -> Result<crate::user_identity::IdentityPubkey, String> {
    imp::unlock(app, window, control).await
}

/// Delete the Keychain item. The account (seed + phrase) is unaffected.
#[tauri::command]
pub async fn touchid_disable() -> Result<(), String> {
    imp::disable()
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn available() -> bool {
        false
    }
    pub fn enrolled() -> bool {
        false
    }
    pub fn enroll(_p: String) -> Result<(), String> {
        Err("Touch ID is only available on macOS".into())
    }
    pub fn disable() -> Result<(), String> {
        Err("Touch ID is only available on macOS".into())
    }
    pub async fn unlock(
        _app: crate::rt::AppHandle,
        _window: crate::rt::WebviewWindow,
        _control: tauri::State<'_, crate::daemon::NodeControl>,
    ) -> Result<crate::user_identity::IdentityPubkey, String> {
        Err("Touch ID is only available on macOS".into())
    }
}

// ── macOS implementation (needs-mac-verify: not compiled on the Linux CI host) ──
//
// The `security-framework` v3 API surface below (PasswordOptions setters,
// SecAccessControl::create_with_protection, the passwords helpers) is confirmed
// against the pinned crate version ON THE MAC during live QA. If the safe
// `set_access_control` setter is absent in the pinned version, drop to
// `security-framework-sys::SecItemAdd` with a CFDictionary carrying
// kSecClass=kSecClassGenericPassword, kSecAttrService/Account, kSecAttrAccessControl
// (from SecAccessControlCreateWithFlags(kSecAttrAccessibleWhenUnlockedThisDeviceOnly,
// kSecAccessControlUserPresence)), kSecUseDataProtectionKeychain=true,
// kSecValueData=<passphrase bytes>.
#[cfg(target_os = "macos")]
mod imp {
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::item::{ItemClass, ItemSearchOptions};
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
        set_generic_password_options, PasswordOptions,
    };

    // kSecAccessControlUserPresence = 1 << 0. The biometric-only
    // BiometryCurrentSet ACL this replaced made the item unreadable with the
    // lid closed (clamshell — the sensor is physically unreachable) and
    // PERMANENTLY invalid after any fingerprint-set change; user-presence
    // keeps the 24-word phrase as recovery without bricking the everyday path.
    const USER_PRESENCE: usize = 1 << 0;

    /// errSecUserCanceled — the user dismissed the OS auth sheet.
    const ERR_SEC_USER_CANCELED: i32 = -128;

    /// errSecMissingEntitlement — ACL items live in the data-protection
    /// keychain, open only to builds signed with an application-identifier
    /// entitlement. Dev/ad-hoc builds land here on every `SecItemAdd`.
    const ERR_SEC_MISSING_ENTITLEMENT: i32 = -34018;

    /// LAErrorUserCancel — the user dismissed the LAContext sheet.
    const LA_ERROR_USER_CANCEL: isize = -2;

    /// The rung-1 access control: user presence, this-device-only.
    fn user_presence_ac() -> Result<SecAccessControl, String> {
        SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            USER_PRESENCE,
        )
        .map_err(|e| format!("access-control: {e}"))
    }

    /// User-presence auth is usable iff we can build its access control.
    pub fn available() -> bool {
        user_presence_ac().is_ok()
    }

    /// Non-prompting: search for the item WITHOUT loading its data (no
    /// `load_data`), so the OS never shows a biometric prompt. Present ⇒ enrolled.
    pub fn enrolled() -> bool {
        has_item(super::KEYCHAIN_ACCOUNT) || has_item(super::KEYCHAIN_ACCOUNT_PLAIN)
    }

    fn has_item(account: &str) -> bool {
        ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(super::KEYCHAIN_SERVICE)
            .account(account)
            .search()
            .is_ok()
    }

    pub fn enroll(passphrase: String) -> Result<(), String> {
        // Build the ACL before deleting the existing item: an ACL failure must
        // never leave the user with the old item already gone.
        let ac = user_presence_ac()?;
        let _ = disable(); // idempotent re-enroll (clears both item kinds)
        let mut options =
            PasswordOptions::new_generic_password(super::KEYCHAIN_SERVICE, super::KEYCHAIN_ACCOUNT);
        options.set_access_control(ac);
        match set_generic_password_options(passphrase.as_bytes(), options) {
            Ok(()) => Ok(()),
            // Rung 2: this build can't touch the data-protection keychain, so
            // store a plain login-keychain item; `unlock` compensates by
            // demanding user presence through LAContext before reading it.
            Err(e) if e.code() == ERR_SEC_MISSING_ENTITLEMENT => set_generic_password(
                super::KEYCHAIN_SERVICE,
                super::KEYCHAIN_ACCOUNT_PLAIN,
                passphrase.as_bytes(),
            )
            .map_err(|e| format!("keychain add (plain): {e}")),
            Err(e) => Err(format!("keychain add: {e}")),
        }
    }

    pub fn disable() -> Result<(), String> {
        // not-found is success for a disable.
        for account in [super::KEYCHAIN_ACCOUNT, super::KEYCHAIN_ACCOUNT_PLAIN] {
            let _ = delete_generic_password(super::KEYCHAIN_SERVICE, account);
        }
        Ok(())
    }

    /// Obtain the passphrase bytes, demanding user presence on whichever rung
    /// the item lives at: an ACL item's read makes the OS prompt itself; the
    /// plain fallback item is gated through `user_presence_check`. The
    /// non-prompting `has_item` comes first so "not enrolled at all" never
    /// pops an auth sheet.
    fn read_passphrase() -> Result<Vec<u8>, String> {
        match get_generic_password(super::KEYCHAIN_SERVICE, super::KEYCHAIN_ACCOUNT) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.code() == ERR_SEC_USER_CANCELED => Err("touchid-canceled".to_string()),
            Err(_) if has_item(super::KEYCHAIN_ACCOUNT_PLAIN) => {
                user_presence_check()?;
                get_generic_password(super::KEYCHAIN_SERVICE, super::KEYCHAIN_ACCOUNT_PLAIN)
                    .map_err(|_| "touchid-unavailable".to_string())
            }
            Err(_) => Err("touchid-unavailable".to_string()),
        }
    }

    /// The rung-2 user-presence gate: block on an LAContext
    /// `deviceOwnerAuthentication` sheet. The reply block fires on
    /// LocalAuthentication's own queue; we're on a NodeControl worker thread,
    /// so blocking on the channel is safe.
    fn user_presence_check() -> Result<(), String> {
        use block2::RcBlock;
        use objc2_foundation::{NSError, NSString};
        use objc2_local_authentication::{LAContext, LAPolicy};

        let (tx, rx) = std::sync::mpsc::channel();
        let ctx;
        unsafe {
            ctx = LAContext::new();
            let reason = NSString::from_str("unlock your Ducktape account");
            let reply = RcBlock::new(move |ok: objc2::runtime::Bool, err: *mut NSError| {
                let code = if err.is_null() { 0 } else { (*err).code() };
                let _ = tx.send((ok.as_bool(), code));
            });
            ctx.evaluatePolicy_localizedReason_reply(
                LAPolicy::DeviceOwnerAuthentication,
                &reason,
                &reply,
            );
        }
        // The context must outlive the wait: releasing an LAContext can
        // cancel its in-flight evaluation (and tear down the sheet).
        let outcome = rx.recv();
        drop(ctx);
        match outcome {
            Ok((true, _)) => Ok(()),
            Ok((false, LA_ERROR_USER_CANCEL)) => Err("touchid-canceled".to_string()),
            Ok((false, code)) => Err(format!("touchid-unavailable (LAError {code})")),
            Err(_) => Err("touchid-unavailable".to_string()),
        }
    }

    pub async fn unlock(
        app: crate::rt::AppHandle,
        window: crate::rt::WebviewWindow,
        control: tauri::State<'_, crate::daemon::NodeControl>,
    ) -> Result<crate::user_identity::IdentityPubkey, String> {
        crate::daemon::require_main_window(&window)?;
        let control = control.inner().clone();
        control
            .run(move || {
                // "touchid-canceled" is a dismissed sheet — not a failure;
                // "touchid-unavailable" is the sentinel the frontend maps to
                // "use your password or recovery phrase."
                let bytes = read_passphrase()?;
                let pass =
                    String::from_utf8(bytes).map_err(|_| "corrupt keychain item".to_string())?;
                let pubkey = crate::user_identity::unlock_with_secret(
                    &app,
                    crate::user_identity::SecretString::new(pass.clone()),
                )?;
                // Re-enroll under the current best rung: migrates biometric-only
                // items from before the user-presence switch, and upgrades a
                // plain fallback item to a real ACL item the first time an
                // entitled (signed) build unlocks it — a successful unlock is
                // the one moment the passphrase is in hand. Deliberately
                // unconditional (every unlock): an item's ACL can't be read
                // back cheaply, and delete+add never prompts. Best-effort —
                // the vault is already unlocked either way.
                let _ = enroll(pass);
                Ok(pubkey)
            })
            .await
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn available_is_false_off_macos() {
        assert!(!touchid_available().await);
    }

    #[tokio::test]
    async fn enrolled_is_false_off_macos() {
        assert!(!touchid_enrolled().await);
    }

    #[tokio::test]
    async fn enroll_errs_off_macos() {
        assert!(touchid_enroll("x".into()).await.is_err());
    }

    #[tokio::test]
    async fn disable_errs_off_macos() {
        assert!(touchid_disable().await.is_err());
    }
}

// Round-trips a throwaway *non-biometric* generic-password item to prove the
// add/copy/delete plumbing and CFString keys are correct on macOS. The
// biometric-ACL path (real enroll/unlock) is manual-only — Touch ID cannot be
// driven in CI. (needs-mac-verify: not compiled on the Linux host.)
#[cfg(all(test, target_os = "macos"))]
mod macos_tests {
    use security_framework::passwords::{
        delete_generic_password, get_generic_password, set_generic_password,
    };

    #[test]
    fn generic_password_round_trips() {
        let svc = "com.ducktape.app.test-roundtrip";
        let acct = "t";
        let _ = delete_generic_password(svc, acct);
        set_generic_password(svc, acct, b"secret").unwrap();
        assert_eq!(get_generic_password(svc, acct).unwrap(), b"secret");
        delete_generic_password(svc, acct).unwrap();
        assert!(get_generic_password(svc, acct).is_err());
    }
}
