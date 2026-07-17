// Typed client for the node's `identity` module — the TS mirror of
// `crates/system/identity/src/interface.rs`. An ACCOUNT is the umbrella a
// person owns: keyed by its founding key (account_id = the first member key),
// it collects many MEMBER KEYS of different schemes (an ed25519 seed key, a
// WebAuthn passkey, a native P-256 key), shares one display name, and owns many
// NODES. Every state op is authorized by a member key; BindNode/UnbindNode and
// the AddMemberKey/RemoveMemberKey ops carry a MemberAuth minted by the native
// shell over a chain-and-nonce-scoped preimage — this client never signs, it
// only forwards the ready-to-submit msg JSON via `submitRawMsg`. SetAccountName
// is origin-gated (a bound node is user-trusted hardware). Pure functions over
// an injected NodeTransport, camelCase params in, verbatim serde wire out.

import { hexToBytes } from "./agent-client";
import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (IdentityReply payloads, verbatim) ────────

/** The scheme of a member key, serde snake_case verbatim from `KeyKind`. */
export type KeyKind = "ed25519" | "p256" | "webauthn_p256";

export interface MemberKeyView {
  pubkey: number[];
  kind: KeyKind;
  label: string | null;
  added_at: number;
}

/** One bound node as the identity module exposes it: the node key plus the
 *  optional on-chain device label (`SetNodeLabel`). */
export interface NodeView {
  node_key: number[];
  label: string | null;
}

export interface AccountView {
  account_id: number[];
  display_name: string | null;
  /** Account avatar reference — a duckfs path (`/shared/attachments/avatars/…`)
   *  the app resolves against the files module, or null when unset. Global to
   *  the account; the app's reconcile-on-connect pass propagates it per-network. */
  avatar: string | null;
  /** Account bio/status line, or null when unset. Global to the account. */
  bio: string | null;
  nonce: number;
  member_keys: MemberKeyView[];
  nodes: NodeView[];
  updated_at: number;
}

export const TARGET = "identity";

/** Query page bound mirrored from the interface crate (MAX_QUERY_LIMIT). */
export const MAX_QUERY_LIMIT = 256;

// ── Hex helper ───────────────────────────────────────────
//
// wire.ts carries only reply-decoding; the hex<->bytes converter this client
// needs (node/account/member keys arrive as hex strings from the UI, go out as
// byte arrays on the wire) already lives on agent-client — same re-export
// pattern chat-client uses for its `keyBytes`.
export { hexToBytes };

// ── Msgs (writes) ────────────────────────────────────────

/** Forward a native-signed IdentityMsg (BindNode/UnbindNode/AddMemberKey/
 *  RemoveMemberKey) untouched: the shell mints the MemberAuth over the signed
 *  preimage, this client only parses and submits the resulting one-line JSON. */
export const submitRawMsg = (
  transport: NodeTransport,
  msgJson: string,
): Promise<BlockEvent> => transport.submit(TARGET, JSON.parse(msgJson));

export const setAccountName = (
  transport: NodeTransport,
  params: { displayName: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_account_name: { display_name: params.displayName } },
    params.origin,
  );

/** Set the account's avatar reference and/or bio. Origin-gated exactly like
 *  `setAccountName` (a bound node is user-trusted hardware) — the shell never
 *  signs. `null` clears a field. `avatar` is a duckfs path already committed to
 *  the files plane; the identity module stores only the reference. */
export const setAccountProfile = (
  transport: NodeTransport,
  params: { avatar: string | null; bio: string | null; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { set_profile: { avatar: params.avatar, bio: params.bio } },
    params.origin,
  );

/** Set (or clear, on empty) the on-chain label of a bound node. Origin-gated
 *  exactly like `setAccountName`: the local managed node stamps its own key as
 *  origin, and the module accepts the label only for a node bound to that same
 *  account — no signing ceremony (mirrors the account-name write). */
export const setNodeLabel = (
  transport: NodeTransport,
  params: { nodeKeyHex: string; label: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    set_node_label: {
      node_key: hexToBytes(params.nodeKeyHex),
      label: params.label,
    },
  });

// ── Queries (reads over committed state) ────────────────

/** Every account, ascending by account id. */
export const allAccounts = (
  transport: NodeTransport,
  { from = 0, limit = MAX_QUERY_LIMIT }: { from?: number; limit?: number } = {},
): Promise<AccountView[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { all: { from, limit } }))
    .then((reply) => replyVariant<AccountView[]>(reply, "accounts"));

/** One account by its id (its founding key, hex). */
export const getAccount = (
  transport: NodeTransport,
  accountIdHex: string,
): Promise<AccountView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { get: { account_id: hexToBytes(accountIdHex) } }),
    )
    .then((reply) => replyVariant<AccountView | null>(reply, "account"));

/** The account owning `nodeKeyHex`, if bound — the resolver other modules and
 *  the app read through. */
export const accountOfNode = (
  transport: NodeTransport,
  nodeKeyHex: string,
): Promise<AccountView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { of_node: { node_key: hexToBytes(nodeKeyHex) } }),
    )
    .then((reply) => replyVariant<AccountView | null>(reply, "account"));

/** The account a `memberKeyHex` belongs to, if any — how a device finds its own
 *  account from whatever member key it holds locally. */
export const accountOfMember = (
  transport: NodeTransport,
  memberKeyHex: string,
): Promise<AccountView | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        of_member: { member_key: hexToBytes(memberKeyHex) },
      }),
    )
    .then((reply) => replyVariant<AccountView | null>(reply, "account"));

const bytesToHex = (bytes: number[]): string =>
  bytes.map((b) => b.toString(16).padStart(2, "0")).join("");

/** ADR A5 ownership: is `accountKeyHex` a member of the account that owns
 *  `nodeKeyHex`? This is the chain fact behind remote node control — the app is
 *  the owner iff its logged-in account key is a member of the node's bound
 *  account. False when the node is unbound, the key is empty, or the read fails
 *  (fail-closed: no ownership claim without a committed one). */
export const nodeOwnedBy = (
  transport: NodeTransport,
  nodeKeyHex: string,
  accountKeyHex: string,
): Promise<boolean> => {
  if (!accountKeyHex || !nodeKeyHex) return Promise.resolve(false);
  const want = accountKeyHex.toLowerCase();
  return accountOfNode(transport, nodeKeyHex)
    .then(
      (account) =>
        !!account && account.member_keys.some((m) => bytesToHex(m.pubkey) === want),
    )
    .catch(() => false);
};
