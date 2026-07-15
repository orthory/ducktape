// Typed client for this machine's user identity (`~/.ducktape/user.key`) —
// the static web twin of the native identity service. Every mutator is a
// native call, so these ONLY work in the desktop build: the web build has no
// local user key file and no `user-key` verbs to shell out to. `identityState`
// is the one exception — it resolves an inert "absent" shape on the web build
// instead of rejecting, so the identity gate can call it unconditionally; every
// mutator below rejects with a clear error there instead.
//
// Wire casing is the Rust commands' own (`#[serde(rename_all = "camelCase")]`
// on every reply shape and the native bridge's camelCase arg convention), so these TS
// shapes and call sites match verbatim — no remapping.

import { hasNativeShell, nativeCall as invoke } from "./node-bootstrap";

// ── Wire types (verbatim from user_identity.rs) ─────────

/** The shell→webview event fired whenever the session password cache stores a
 *  verified password (create/restore/unlock/encrypt/reveal — Touch ID
 *  included): the moment signing becomes possible. Mirrors
 *  `IDENTITY_UNLOCKED_EVENT` in user_identity.rs; the console provider listens
 *  and re-runs the connect-time auto-bind, which the boot connect otherwise
 *  loses to the human-speed unlock on every launch. */
export const IDENTITY_UNLOCKED_EVENT = "ducktape://identity-unlocked";

/** The identity gate's state machine value:
 *  - "absent": no `user.key` on disk yet — onboarding must create or restore one.
 *  - "plaintext": a legacy (pre-encryption) key — signs freely, no password.
 *  - "locked": an encrypted key with no verified password cached this session.
 *  - "unlocked": an encrypted key whose password IS cached this session. */
export type IdentityState = "absent" | "plaintext" | "locked" | "unlocked";

/** [`identityState`]'s reply shape. */
export interface IdentityStateReport {
  state: IdentityState;
  /** Absent when there is no key on disk yet. */
  pubkey?: string;
  /** The UX-only "confirmed the recovery phrase once" registry flag. */
  mnemonicConfirmed: boolean;
}

/** [`createIdentity`]'s success shape: the mnemonic is shown exactly once —
 *  the caller owns prompting the confirm-3-words step, then [`confirmMnemonic`]
 *  (re-fetching it later is [`revealMnemonic`], which always re-prompts). */
export interface IdentityCreated {
  pubkey: string;
  mnemonic: string;
}

/** The pubkey-only success shape ([`restoreIdentity`], [`unlockIdentity`],
 *  [`encryptLegacy`]). */
export interface IdentityPubkey {
  pubkey: string;
}

/** [`revealMnemonic`]'s success shape. */
export interface IdentityMnemonic {
  mnemonic: string;
}

// ── Guard ────────────────────────────────────────────────

/** Every mutator below rejects with this on the web build: there is no local
 *  user key file and no `user-key` verbs to shell out to. */
const notDesktop = <T>(): Promise<T> =>
  Promise.reject(
    new Error("user identity is available in the desktop app only"),
  );

// ── Reads ────────────────────────────────────────────────

/** The identity gate's input: folds `user-key status` with the shell's
 *  session password cache and the registry's mnemonic-confirmed flag. The web
 *  build has no local user key at all — resolves the inert "absent" shape
 *  without invoking, so callers can gate on this everywhere unconditionally. */
export const identityState = (): Promise<IdentityStateReport> => {
  if (!hasNativeShell()) {
    return Promise.resolve({ state: "absent", mnemonicConfirmed: true });
  }
  return invoke<IdentityStateReport>("user_identity_state");
};

// ── Writes ───────────────────────────────────────────────

/** Create a brand-new identity: mints a fresh seed encrypted with `password`,
 *  returning the pubkey and the mnemonic. Leaves mnemonic-confirmed false —
 *  the caller still owes the user the confirm-3-words step. */
export const createIdentity = (password: string): Promise<IdentityCreated> =>
  hasNativeShell()
    ? invoke<IdentityCreated>("user_identity_create", { password })
    : notDesktop();

/** Restore an identity from its 24-word mnemonic, encrypted with a new
 *  `password`. Marks mnemonic-confirmed true (typing the words back in
 *  counts as confirming them). */
export const restoreIdentity = (
  mnemonic: string,
  password: string,
): Promise<IdentityPubkey> =>
  hasNativeShell()
    ? invoke<IdentityPubkey>("user_identity_restore", { mnemonic, password })
    : notDesktop();

/** Unlock an existing encrypted identity for this session — pure
 *  verification, nothing persists on disk; caches `password` in the shell's
 *  process memory for the rest of the app's run on success. */
export const unlockIdentity = (password: string): Promise<IdentityPubkey> =>
  hasNativeShell()
    ? invoke<IdentityPubkey>("user_identity_unlock", { password })
    : notDesktop();

/** Reveal the 24-word mnemonic. ALWAYS re-verifies `password` fresh — the
 *  session cache is never consulted here, by design: this is the one action
 *  that must always re-prompt, however recently the identity was unlocked.
 *  (A successful reveal still STORES the just-verified password, so finishing
 *  the resume-create ceremony leaves the session unlocked.) */
export const revealMnemonic = (password: string): Promise<IdentityMnemonic> =>
  hasNativeShell()
    ? invoke<IdentityMnemonic>("user_identity_reveal", { password })
    : notDesktop();

/** Migrate a legacy plaintext identity to encrypted (v2), in place. */
export const encryptLegacy = (password: string): Promise<IdentityPubkey> =>
  hasNativeShell()
    ? invoke<IdentityPubkey>("user_identity_encrypt", { password })
    : notDesktop();

/** Mark the identity-creation mnemonic confirmed — a persisted, UX-only
 *  Registry flag with no security weight (it only stops the identity gate
 *  from re-showing the confirmation step on future launches). */
export const confirmMnemonic = (): Promise<void> =>
  hasNativeShell() ? invoke<void>("user_identity_confirm_mnemonic") : notDesktop();

/** Drop the session-cached password (a Settings "lock" affordance). The next
 *  bind/unbind (or any signing call) on an encrypted key needs a fresh
 *  unlock, or fails with `"identity-locked"`. */
export const lockIdentity = (): Promise<void> =>
  hasNativeShell() ? invoke<void>("user_identity_lock") : notDesktop();
