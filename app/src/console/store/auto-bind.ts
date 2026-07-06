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

import { getUser, submitRawMsg, userOf } from "../../domain/identity-client";
import { isTauri } from "../../domain/node-bootstrap";
import type { NodeTransport } from "../../domain/transport";

export type AutoBindResult = "bound" | "already" | "skipped" | "failed";

export const autoBindUserIdentity = (
  transport: NodeTransport,
  workspace: { chainId: string; pubkey: string },
): Promise<AutoBindResult> => {
  // Web build (or any non-desktop shell): there is no machine user key to
  // offer and no `user_sign_bind` command to sign with.
  if (!isTauri()) return Promise.resolve("skipped");

  return Promise.resolve()
    .then(() => userOf(transport, workspace.pubkey))
    .then((bound) => {
      if (bound) return "already" as const;
      return invoke<{ pubkey: string }>("user_identity_status")
        .then(({ pubkey: userKey }) =>
          getUser(transport, userKey).then((user) => user?.nonce ?? 0),
        )
        .then((nonce) =>
          invoke<string>("user_sign_bind", {
            chainId: workspace.chainId,
            nodePub: workspace.pubkey,
            nonce,
          }),
        )
        .then((msg) => submitRawMsg(transport, msg))
        .then(() => "bound" as const);
    })
    .catch((): AutoBindResult => "failed");
};
