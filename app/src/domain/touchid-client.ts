// Typed client for native macOS Touch ID custody — the TS mirror of
// app/src-tauri/src/touchid.rs. Touch ID stores the vault passphrase behind a
// biometric-ACL Keychain item; these are thin `invoke` wrappers, so the
// mutators ONLY work in the desktop macOS build. The two presence checks
// (`touchidAvailable`, `touchidEnrolled`) resolve `false` off-Tauri without
// invoking, so the UI can gate Touch ID affordances unconditionally.

import { invoke } from "@tauri-apps/api/core";

import { isTauri } from "./node-bootstrap";

/** A random 32-byte passphrase, base64. Never shown to the user, never
 *  persisted in JS — it encrypts the vault and is handed straight to the
 *  Keychain. Recovery is the 24-word phrase. */
export const randomPassphrase = (): string => {
  const b = new Uint8Array(32);
  crypto.getRandomValues(b);
  return btoa(String.fromCharCode(...b));
};

/** True only on macOS with a usable biometric authenticator. Gates every
 *  piece of Touch ID UI; false everywhere off the desktop macOS build. */
export const touchidAvailable = (): Promise<boolean> =>
  isTauri() ? invoke<boolean>("touchid_available") : Promise.resolve(false);

/** Whether a Touch ID Keychain item exists for this account — a non-prompting
 *  presence check (no biometric dialog). False off the desktop build. */
export const touchidEnrolled = (): Promise<boolean> =>
  isTauri() ? invoke<boolean>("touchid_enrolled") : Promise.resolve(false);

/** Store the vault passphrase behind a biometric ACL. Called once, right after
 *  the recovery phrase is confirmed. */
export const touchidEnroll = (passphrase: string): Promise<void> =>
  invoke<void>("touchid_enroll", { passphrase });

/** Prompt Touch ID, retrieve the passphrase, unlock the vault, cache it.
 *  Rejects with the `touchid-unavailable` sentinel when the item is gone —
 *  callers map that to the recovery-phrase path. */
export const touchidUnlock = (): Promise<{ pubkey: string }> =>
  invoke<{ pubkey: string }>("touchid_unlock");

/** Delete the Keychain item. The account (seed + phrase) is unaffected. */
export const touchidDisable = (): Promise<void> =>
  invoke<void>("touchid_disable");
