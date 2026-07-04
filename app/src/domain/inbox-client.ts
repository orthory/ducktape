// Typed client for the node's `inbox` module — the TS mirror of
// `crates/apps/inbox-interface`. The inbox holds per-member notification queues
// as consensus state: other modules deliver notifications as follow-up ops, so a
// note commits atomically with the event that caused it, and no external push
// service is involved (the air-gap-native notification story).
//
// `member` is an OPAQUE member-identity string chosen by whoever delivers; the
// console keys a member's queue by the local author identity. `source` (the
// DELIVERING origin) is derived by the module from the submit origin, never
// caller-supplied — so it never appears in a write here.
//
// Same contract as tasks-client: camelCase params in, verbatim serde wire out,
// pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (Notification records + InboxReply payloads, verbatim) ─

export interface Notification {
  seq: number;
  member: string;
  kind: string;
  body: string;
  /** The delivering origin: a module id verbatim, `ext:<hex>` for an external
   *  submitter, or `system`. Set by the module, never on a write. */
  source: string;
  created_at: number;
  read: boolean;
}

const TARGET = "inbox";

/** Query page ceiling (mirrors MAX_QUERY_LIMIT); larger limits are clamped. */
export const MAX_QUERY_LIMIT = 256;

// ── Msgs (writes) ───────────────────────────────────────

/** Enqueue a notification for `member`. Accepted from any origin — a submitter
 *  may self-deliver a note; the module stamps `source` from the origin. */
export const deliver = (
  transport: NodeTransport,
  params: { member: string; kind: string; body: string; origin?: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { Deliver: { member: params.member, kind: params.kind, body: params.body } },
    params.origin,
  );

/** Mark every item with `seq <= upToSeq` as read (idempotent). */
export const markRead = (
  transport: NodeTransport,
  params: { member: string; upToSeq: number },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    MarkRead: { member: params.member, up_to_seq: params.upToSeq },
  });

/** Delete every item with `seq <= upToSeq`. `next_seq` never rewinds. */
export const clear = (
  transport: NodeTransport,
  params: { member: string; upToSeq: number },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    Clear: { member: params.member, up_to_seq: params.upToSeq },
  });

// ── Queries (reads over committed state) ────────────────

/** Items for `member`, ascending by seq starting at `fromSeq`, at most `limit`
 *  (clamped to MAX_QUERY_LIMIT). */
export const list = (
  transport: NodeTransport,
  params: { member: string; fromSeq?: number; limit?: number },
): Promise<Notification[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        List: {
          member: params.member,
          from_seq: params.fromSeq ?? 0,
          limit: params.limit ?? MAX_QUERY_LIMIT,
        },
      }),
    )
    .then((reply) => replyVariant<Notification[]>(reply, "Items"));

/** Count of unread items for `member`. */
export const unread = (
  transport: NodeTransport,
  member: string,
): Promise<number> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { Unread: { member } }))
    .then((reply) => replyVariant<number>(reply, "UnreadCount"));
