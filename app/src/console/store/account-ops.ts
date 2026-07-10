// The Account view's writes: mint a device-link challenge, approve a link
// response (AddMemberKey), drop a member key, and unbind a node. All signing
// happens in the tauri shell (`user_sign_*` verbs) over this machine's
// account key — this module only assembles inputs from fresh account facts
// read through the live transport, forwards the ready-to-submit msg JSON, and
// maps the shell's `identity-locked` sentinel into an actionable error. Deps
// are injected so tests drive a stub transport and a mocked invoke.
//
// Nonce discipline: every signed identity op is scoped to the account's
// CURRENT nonce. The link ceremony spans two devices and a paste buffer, so
// the possession proof inside a response is pinned to the nonce the challenge
// was minted at — approve therefore re-reads the account and refuses on
// drift (any account op landing in between invalidates the proof) instead of
// submitting a doomed msg.

import { invoke } from "@tauri-apps/api/core";

import { keyHex } from "../../domain/chat-client";
import { enrollStart } from "../../domain/enroll-client";
import { accountOfNode, hexToBytes, submitRawMsg } from "../../domain/identity-client";
import type { AccountView } from "../../domain/identity-client";
import type { NodeTransport } from "../../domain/transport";
import { decodeLinkResponse } from "../views/account/link-device";
import type { LinkChallenge } from "../views/account/link-device";

export interface AccountOpsDeps {
  transport: NodeTransport;
  chainId: string;
  /** The active workspace's node key (hex) — the anchor the account is
   *  resolved through (`of_node`). */
  nodePub: string;
}

const rethrowActionable = (err: unknown): never => {
  const message = err instanceof Error ? err.message : String(err);
  throw new Error(
    message === "identity-locked"
      ? "your account is locked on this device — unlock it first, then retry"
      : message,
  );
};

const ownAccount = (deps: AccountOpsDeps): Promise<AccountView> =>
  accountOfNode(deps.transport, deps.nodePub).then((account) => {
    if (!account) throw new Error("this node isn't linked to an account yet");
    return account;
  });

/** Fresh link challenge for this account — the caller encodes it for display
 *  and keeps the object to approve the response against. */
export const mintLinkChallenge = (deps: AccountOpsDeps): Promise<LinkChallenge> =>
  ownAccount(deps).then((account) => ({
    chainId: deps.chainId,
    accountId: keyHex(account.account_id),
    nonce: account.nonce,
    name: account.display_name,
  }));

/** Approve a pasted link response: authorize the new member key and submit
 *  `AddMemberKey`. Refuses on nonce drift (see the module header). */
export const addMemberFromResponse = (
  deps: AccountOpsDeps,
  challenge: LinkChallenge,
  responseBlob: string,
): Promise<void> => {
  const response = decodeLinkResponse(responseBlob);
  if (!response) {
    return Promise.reject(
      new Error("that doesn't look like a link response code — paste the code from the new device"),
    );
  }
  return ownAccount(deps).then((account) => {
    if (account.nonce !== challenge.nonce) {
      throw new Error(
        "the account changed since this link code was made — re-run the link from step 1",
      );
    }
    return invoke<string>("user_sign_add_member", {
      chainId: deps.chainId,
      accountId: challenge.accountId,
      newPub: response.pubkey,
      newKind: response.kind,
      nonce: challenge.nonce,
      possession: response.possession,
      label: response.label,
    })
      .then((msg) => submitRawMsg(deps.transport, msg))
      .then(() => undefined, rethrowActionable);
  });
};

/** Drop `targetKeyHex` from this account (any member may drop any member;
 *  the module itself refuses to drop the last remaining key). */
export const removeMemberKey = (
  deps: AccountOpsDeps,
  targetKeyHex: string,
): Promise<void> =>
  ownAccount(deps).then((account) =>
    invoke<string>("user_sign_remove_member", {
      chainId: deps.chainId,
      accountId: keyHex(account.account_id),
      targetKey: targetKeyHex,
      nonce: account.nonce,
    })
      .then((msg) => submitRawMsg(deps.transport, msg))
      .then(() => undefined, rethrowActionable),
  );

// ── Phone enrollment (the LAN QR ceremony, enroll.rs) ────
//
// Same nonce discipline as the paste ceremony above: the phone signs a
// payload pinned to the nonce `enroll_start` was called at, so approve
// re-reads the account and refuses on drift instead of submitting a doomed
// msg. The desktop stays the authority — the LAN server only relays a
// CANDIDATE key; nothing lands until approve signs the authorizer here.

/** One in-flight phone enrollment: the QR url plus the account facts the
 *  phone's possession proof is pinned to. */
export interface PhoneEnrollment {
  url: string;
  accountId: string;
  nonce: number;
}

/** Stand up the LAN enrollment server for this account (fresh nonce) and
 *  return the QR url + the pinned facts approve verifies against. */
export const startPhoneEnrollment = (deps: AccountOpsDeps): Promise<PhoneEnrollment> =>
  ownAccount(deps).then((account) => {
    const accountId = keyHex(account.account_id);
    return enrollStart(deps.chainId, accountId, account.nonce).then(({ url }) => ({
      url,
      accountId,
      nonce: account.nonce,
    }));
  });

/** Approve the phone's candidate key: authorize + submit `AddMemberKey` with
 *  the P-256 possession (`MemberProof::Signature`, raw R‖S bytes). */
export const approvePhoneEnrollment = (
  deps: AccountOpsDeps,
  enrollment: PhoneEnrollment,
  newKeyHex: string,
  sigHex: string,
  label: string | null,
): Promise<void> =>
  ownAccount(deps).then((account) => {
    if (account.nonce !== enrollment.nonce) {
      throw new Error(
        "the account changed since this QR was made — re-run the enrollment",
      );
    }
    // The wire shape verify_authority expects for a native P-256 key:
    // externally-tagged MemberProof, sig as raw bytes.
    const possession = JSON.stringify({ signature: { sig: hexToBytes(sigHex) } });
    return invoke<string>("user_sign_add_member", {
      chainId: deps.chainId,
      accountId: enrollment.accountId,
      newPub: newKeyHex,
      newKind: "p256",
      nonce: enrollment.nonce,
      possession,
      label,
    })
      .then((msg) => submitRawMsg(deps.transport, msg))
      .then(() => undefined, rethrowActionable);
  });

/** Evict `targetNodeHex` from this account — the lost-device affordance. The
 *  node keeps running; it just stops being yours (and the nonce bump kills
 *  any captured bind certificates). */
export const unbindNode = (
  deps: AccountOpsDeps,
  targetNodeHex: string,
): Promise<void> =>
  ownAccount(deps).then((account) =>
    invoke<string>("user_sign_unbind", {
      chainId: deps.chainId,
      nodePub: targetNodeHex,
      nonce: account.nonce,
    })
      .then((msg) => submitRawMsg(deps.transport, msg))
      .then(() => undefined, rethrowActionable),
  );
