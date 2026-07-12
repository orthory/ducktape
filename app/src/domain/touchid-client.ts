// Typed client for native macOS Touch ID custody — the TS mirror of
// app/src-tauri/src/touchid.rs. Every mutator is a Tauri `invoke`, so they
// only do anything in the desktop build; `touchidAvailable`/`touchidEnrolled`
// resolve `false` on the web build (and off macOS) so the UI can probe
// unconditionally and simply hide the Touch ID affordances.

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

export const touchidAvailable = (): Promise<boolean> =>
  isTauri() ? invoke<boolean>("touchid_available") : Promise.resolve(false);

/** Non-prompting presence check — is a Touch ID item enrolled on this device? */
export const touchidEnrolled = (): Promise<boolean> =>
  isTauri() ? invoke<boolean>("touchid_enrolled") : Promise.resolve(false);

export const touchidEnroll = (passphrase: string): Promise<void> =>
  invoke<void>("touchid_enroll", { passphrase });

export const touchidUnlock = (): Promise<{ pubkey: string }> =>
  invoke<{ pubkey: string }>("touchid_unlock");

export const touchidDisable = (): Promise<void> => invoke<void>("touchid_disable");
