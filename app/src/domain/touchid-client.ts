// Typed client for native macOS Touch ID custody — the TS mirror of
// the native desktop backend. Touch ID stores the vault passphrase behind a
// user-presence-ACL Keychain item (Touch ID when the sensor is usable, the
// Mac's login password when it isn't); these are thin `invoke` wrappers, so the
// mutators ONLY work in the desktop macOS build. The two presence checks
// (`touchidAvailable`, `touchidEnrolled`) resolve `false` outside the desktop without
// invoking, so the UI can gate Touch ID affordances unconditionally.

import { hasNativeShell, nativeCall as invoke } from "./node-bootstrap";

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
  hasNativeShell() ? invoke<boolean>("touchid_available") : Promise.resolve(false);

/** Whether a Touch ID Keychain item exists for this account — a non-prompting
 *  presence check (no biometric dialog). False off the desktop build. */
export const touchidEnrolled = (): Promise<boolean> =>
  hasNativeShell() ? invoke<boolean>("touchid_enrolled") : Promise.resolve(false);

/** Store the vault passphrase behind a biometric ACL. Called once, right after
 *  the recovery phrase is confirmed. */
export const touchidEnroll = (passphrase: string): Promise<void> =>
  invoke<void>("touchid_enroll", { passphrase });

/** Prompt the OS user-presence sheet (Touch ID when usable, login password
 *  otherwise), retrieve the passphrase, unlock the vault, cache it. Rejects
 *  with the `touchid-canceled` sentinel when the user dismisses the sheet
 *  (not an error) and with `touchid-unavailable` when the item is gone —
 *  callers map the latter to the password/recovery-phrase paths. */
export const touchidUnlock = (): Promise<{ pubkey: string }> =>
  invoke<{ pubkey: string }>("touchid_unlock");

/** Delete the Keychain item. The account (seed + phrase) is unaffected. */
export const touchidDisable = (): Promise<void> =>
  invoke<void>("touchid_disable");
