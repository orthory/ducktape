// Thin client over the desktop's in-app LAN enrollment commands (enroll.rs).
//
// enroll_start binds an ephemeral, token-gated LAN server and returns the URL to
// render as a QR; the phone opens it, generates a P-256 key, and posts back a
// signature. enroll_poll returns [newKeyHex, sigHex] once the phone finishes;
// enroll_cancel tears the server down. All desktop-only native calls.

import { nativeCall as invoke } from "./node-bootstrap";

export interface EnrollStart {
  url: string;
}

/** Start enrollment INTO `accountIdHex` at `nonce`; returns the QR URL. */
export const enrollStart = (
  chainId: string,
  accountId: string,
  nonce: number,
): Promise<EnrollStart> =>
  invoke<EnrollStart>("enroll_start", { chainId, accountId, nonce });

/** `[newKeyHex, sigHex]` once the phone has posted its signed proof, else null. */
export const enrollPoll = (): Promise<[string, string] | null> =>
  invoke<[string, string] | null>("enroll_poll");

/** Tear the enrollment server down (on success, cancel, or leaving the screen). */
export const enrollCancel = (): Promise<void> => invoke<void>("enroll_cancel");
