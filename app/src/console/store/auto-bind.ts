// Auto-bind: on a desktop connect, quietly offer this machine's user key to
// bind the workspace's node — so a freshly-created or freshly-joined node
// starts life owned by whoever's tauri shell is running it, with no manual
// "bind" step. Fire-and-forget from the connect action (see actions.ts):
// every step below is wrapped so a failure here NEVER surfaces as a connect
// error — the identity module is best-effort convenience, not a gate, and the
// next connect (or a manual bind) retries. A nonce race between two devices
// binding concurrently resolves here too: one submit lands, the loser's
// `submit` rejects and this resolves "failed" like any other hiccup.

import { invoke } from "@tauri-apps/api/core";

import {
  accountOfMember,
  accountOfNode,
  submitRawMsg,
} from "../../domain/identity-client";
import { isTauri } from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";
import { identityState } from "../../domain/user-identity-client";

export type AutoBindResult =
  | "bound"
  | "already"
  | "skipped"
  | "failed"
  | "locked";

export const autoBindUserIdentity = (
  transport: NodeTransport,
  workspace: { chainId: string; pubkey: string },
): Promise<AutoBindResult> => {
  // Web build (or any non-desktop shell): there is no machine user key to
  // offer and no `user_sign_bind` command to sign with.
  if (!isTauri()) return Promise.resolve("skipped");

  return identityState()
    .then(({ state, pubkey: userKey }) => {
      // Only a readable key can sign a bind. An absent key has nothing to
      // offer yet; an encrypted-and-locked key needs a password this
      // fire-and-forget call never has — signing would just fail with
      // "identity-locked" downstream, so short-circuit here instead of
      // burning a node query on it. The identity gate (onboarding/unlock UI)
      // owns getting the user out of "locked"; the next connect retries.
      if (state !== "unlocked" && state !== "plaintext") {
        return "locked" as const;
      }

      return accountOfNode(transport, workspace.pubkey).then((bound) => {
        if (bound) return "already" as const;
        // Belt-and-suspenders: "unlocked"/"plaintext" always carry a pubkey
        // in the clear (v2 files included), so this should never trip in
        // practice. No pubkey means nothing to sign a bind with, either way.
        if (!userKey) return "failed" as const;
        // The nonce is the ACCOUNT's, resolved via the key's membership —
        // not `getAccount(userKey)`, which only matches when this key is the
        // FOUNDER. Once this machine's key was added to an existing account as
        // a non-founding member, its account is keyed by a different id, so the
        // bind must sign over that account's current nonce. A brand-new key
        // (no account yet) resolves null → nonce 0, and the bind founds it.
        return accountOfMember(transport, userKey)
          .then((account) => account?.nonce ?? 0)
          .then((nonce) =>
            invoke<string>("user_sign_bind", {
              chainId: workspace.chainId,
              nodePub: workspace.pubkey,
              nonce,
            }),
          )
          .then((msg) => submitRawMsg(transport, msg))
          .then(() => "bound" as const);
      });
    })
    .catch((): AutoBindResult => "failed");
};
