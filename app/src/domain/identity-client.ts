// Typed client for the node's `identity` module — the TS mirror of
// `crates/system/identity/src/interface.rs`. A USER is an ed25519 keypair held
// by the person (in the app); a NODE is a workspace's mesh/valset identity.
// BindNode/UnbindNode carry a user-key SIGNATURE minted by the tauri shell
// (`user_sign_bind`/`user_sign_unbind`) over a chain-and-nonce-scoped preimage
// — this client never signs, it only forwards the ready-to-submit msg JSON via
// `submitRawMsg`. SetUserName is origin-gated (a bound node is user-trusted
// hardware), same contract as profiles' SetName. Pure functions over an
// injected NodeTransport, camelCase params in, verbatim serde wire out.

import { hexToBytes } from "./agent-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (IdentityReply payloads, verbatim) ────────

export interface UserView {
  user_key: number[];
  display_name: string | null;
  nonce: number;
  nodes: number[][];
  updated_at: number;
}

export const TARGET = "identity";

/** Query page bound mirrored from the interface crate (MAX_QUERY_LIMIT). */
export const MAX_QUERY_LIMIT = 256;

// ── Hex helper ───────────────────────────────────────────
//
// wire.ts carries only reply-decoding; the hex<->bytes converter this client
// needs (node/user keys arrive as hex strings from the UI, go out as byte
// arrays on the wire) already lives on agent-client — same re-export pattern
// chat-client uses for its `keyBytes`.
export { hexToBytes };

// ── Msgs (writes) ────────────────────────────────────────

/** Forward a tauri-signed IdentityMsg (BindNode/UnbindNode) untouched: the
 *  shell mints `user_sig` over the signed preimage, this client only parses
 *  and submits the resulting one-line JSON. */
export const submitRawMsg = (
  transport: NodeTransport,
  msgJson: string,
): Promise<BlockEvent> => transport.submit(TARGET, JSON.parse(msgJson));

export const setUserName = (
  transport: NodeTransport,
  params: { displayName: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_user_name: { display_name: params.displayName } },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

/** Every user, ascending by user key. */
export const allUsers = (
  transport: NodeTransport,
  { from = 0, limit = MAX_QUERY_LIMIT }: { from?: number; limit?: number } = {},
): Promise<UserView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { all: { from, limit } }))
    .then((reply) => replyVariant<UserView[]>(reply, "users"));

/** One user by user key (hex). */
export const getUser = (
  transport: NodeTransport,
  userKeyHex: string,
): Promise<UserView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { get: { user_key: hexToBytes(userKeyHex) } }),
    )
    .then((reply) => replyVariant<UserView | null>(reply, "user"));

/** The user owning `nodeKeyHex`, if bound — the resolver other modules and the
 *  app read through. */
export const userOf = (
  transport: NodeTransport,
  nodeKeyHex: string,
): Promise<UserView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { user_of: { node_key: hexToBytes(nodeKeyHex) } }),
    )
    .then((reply) => replyVariant<UserView | null>(reply, "user"));
