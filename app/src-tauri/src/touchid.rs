//! Native macOS Touch ID custody: a biometric-ACL Keychain item holding the
//! vault passphrase. No Secure-Enclave key generation, no chain member key —
//! this only changes how `user_identity_unlock`'s passphrase is supplied.
//! Non-macOS targets compile working stubs.
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

/// True only on macOS with a usable biometric authenticator. Gates every
/// piece of Touch ID UI.
#[tauri::command]
pub async fn touchid_available() -> bool {
    imp::available()
}

/// Store the vault passphrase behind a biometry-current-set ACL. Called once,
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

/// Retrieve the passphrase (prompts Touch ID), unlock the vault, cache it.
/// Returns the pubkey, exactly like `user_identity_unlock`.
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
// kSecAccessControlBiometryCurrentSet)), kSecUseDataProtectionKeychain=true,
// kSecValueData=<passphrase bytes>.
#[cfg(target_os = "macos")]
mod imp {
    use security_framework::access_control::{ProtectionMode, SecAccessControl};
    use security_framework::item::{ItemClass, ItemSearchOptions};
    use security_framework::passwords::{set_generic_password_options, PasswordOptions};

    // kSecAccessControlBiometryCurrentSet = 1 << 3. Enrolling behind this flag
    // means REMOVING a fingerprint invalidates the item — the passphrase is
    // never recoverable without the current biometric set (or the 24-word phrase).
    const BIOMETRY_CURRENT_SET: usize = 1 << 3;

    /// A biometric authenticator is usable iff we can build a
    /// biometry-current-set access control; that only succeeds on a platform
    /// with a working biometric ACL.
    pub fn available() -> bool {
        SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            BIOMETRY_CURRENT_SET,
        )
        .is_ok()
    }

    /// Non-prompting: search for the item WITHOUT loading its data (no
    /// `load_data`), so the OS never shows a biometric prompt. Present ⇒ enrolled.
    pub fn enrolled() -> bool {
        ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(super::KEYCHAIN_SERVICE)
            .account(super::KEYCHAIN_ACCOUNT)
            .search()
            .is_ok()
    }

    pub fn enroll(passphrase: String) -> Result<(), String> {
        let _ = disable(); // idempotent re-enroll
        let ac = SecAccessControl::create_with_protection(
            Some(ProtectionMode::AccessibleWhenUnlockedThisDeviceOnly),
            BIOMETRY_CURRENT_SET,
        )
        .map_err(|e| format!("access-control: {e}"))?;
        let mut options =
            PasswordOptions::new_generic_password(super::KEYCHAIN_SERVICE, super::KEYCHAIN_ACCOUNT);
        options.set_access_control(ac);
        set_generic_password_options(passphrase.as_bytes(), options)
            .map_err(|e| format!("keychain add: {e}"))?;
        Ok(())
    }

    pub fn disable() -> Result<(), String> {
        // not-found is success for a disable.
        let _ = security_framework::passwords::delete_generic_password(
            super::KEYCHAIN_SERVICE,
            super::KEYCHAIN_ACCOUNT,
        );
        Ok(())
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
                // Reading the item DATA triggers the OS Touch ID prompt because
                // the item carries a biometric ACL. "touchid-unavailable" is the
                // sentinel the frontend maps to "use your recovery phrase."
                let bytes = security_framework::passwords::get_generic_password(
                    super::KEYCHAIN_SERVICE,
                    super::KEYCHAIN_ACCOUNT,
                )
                .map_err(|_| "touchid-unavailable".to_string())?;
                let pass =
                    String::from_utf8(bytes).map_err(|_| "corrupt keychain item".to_string())?;
                crate::user_identity::unlock_with_secret(
                    &app,
                    crate::user_identity::SecretString::new(pass),
                )
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
