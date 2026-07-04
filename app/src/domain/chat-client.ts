// Typed client for the node's `chat` module — the TS mirror of
// `crates/apps/chat-interface` (the block-based rework).
//
// This file plays the interface crate's role on the client side: it is the
// only place that knows chat's wire shape. Key contract points mirrored here:
//   - authorship is NEVER in a write payload — the module derives it from the
//     block's origin, so write functions take an `origin` and pass it to
//     transport.submit; replies carry `AuthorRef` (a User author holds the
//     origin's raw bytes).
//   - messages are block-based (`ChatBlock` = paragraph/code/quote/divider of
//     `Span`s) and sequence-addressed: a thread reply is a normal message with
//     `thread = root seq`.
//   - queries page by sequence; MessagesLatest returns replies and tombstones
//     too, so the pagination promise stays gap-free — callers filter.
//
// Everything is a pure function over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (verbatim serde shapes) ──────────────────

export type AuthorRef =
  | { User: number[] }
  | { Agent: { module: string; agent_id: string } }
  | { Module: string }
  | "System";

export type Mark = "Bold" | "Italic" | { Link: string } | { Mention: AuthorRef };

export interface Span {
  text: string;
  marks: Mark[];
}

export type ChatBlock =
  | { Paragraph: Span[] }
  | { Code: { lang: string | null; text: string } }
  | { Quote: Span[] }
  | "Divider";

export type PostPolicy = "Open" | "MembersOnly";

export interface Channel {
  id: string;
  name: string;
  created_at: number;
  head_seq: number;
  post_policy: PostPolicy;
  hooks: string[];
  pinned: number[];
}

export interface MessageHead {
  message_id: string;
  author: AuthorRef;
  blocks: ChatBlock[];
  created_at: number;
  rev: number;
  edited_at: number | null;
  base_rev: number | null;
  deleted: boolean;
  thread: number | null;
  reply_count: number;
  last_reply_seq: number | null;
}

export interface ReactionSummary {
  emoji: string;
  reactors: AuthorRef[];
}

export interface MessageView {
  channel_id: string;
  seq: number;
  head: MessageHead;
  reactions: ReactionSummary[];
  channel_head_seq: number;
}

export interface ChatThread {
  root: MessageView;
  replies: MessageView[];
}

const TARGET = "chat";

/** Query page bound mirrored from the interface crate (MAX_QUERY_LIMIT). */
export const MAX_QUERY_LIMIT = 256;

// ── Rendering helpers (wire → display) ──────────────────

/** hex(User key bytes) → display name — the resolved `profiles` registry, keyed
 *  so `authorName` can look a User author up by its origin bytes. */
export type AuthorNames = Record<string, string>;

/** Lowercase hex of a User author's key bytes — the map key into AuthorNames
 *  (and the `profiles` Profile.key, which IS these same origin bytes). */
export const keyHex = (bytes: number[]): string =>
  bytes.map((b) => b.toString(16).padStart(2, "0")).join("");

/** A display name for an author. A User author's bytes are the submitter
 *  identity the daemon stamped; when the `profiles` registry (`names`) resolves
 *  those bytes it wins, else we fall back to the utf-8/hex handle. */
export const authorName = (author: AuthorRef, names?: AuthorNames): string => {
  if (author === "System") return "system";
  if ("User" in author)
    return names?.[keyHex(author.User)] ?? displayUserBytes(author.User);
  if ("Agent" in author) return `${author.Agent.module}/${author.Agent.agent_id}`;
  return author.Module;
};

/** User author bytes are a claimed display name on the embedded daemon but a
 * raw ed25519 pubkey on the networked node (the signed frame origin). Render
 * printable UTF-8 as-is and anything else as a short hex handle until the
 * name registry resolves keys to display names. */
const displayUserBytes = (bytes: number[]): string => {
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(new Uint8Array(bytes));
    // control characters mean "not a name" even when technically valid UTF-8.
    if (!/[\p{Cc}\p{Cn}]/u.test(text)) return text;
  } catch {
    // fall through to the hex handle
  }
  return `${keyHex(bytes).slice(0, 8)}…`;
};

const spanText = (spans: Span[]): string => spans.map((span) => span.text).join("");

/** Flatten blocks to plain text for list rendering. */
export const blocksText = (blocks: ChatBlock[]): string =>
  blocks
    .map((block) => {
      if (block === "Divider") return "———";
      if ("Paragraph" in block) return spanText(block.Paragraph);
      if ("Quote" in block) return `> ${spanText(block.Quote)}`;
      return block.Code.text;
    })
    .join("\n");

// ── Msgs (writes — one submit = one block) ──────────────

export const createChannel = (
  transport: NodeTransport,
  params: { channelId: string; name: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      CreateChannel: {
        channel_id: params.channelId,
        name: params.name,
        post_policy: "Open",
      },
    },
    params.origin,
  );

export const postMessage = (
  transport: NodeTransport,
  params: {
    channelId: string;
    messageId: string;
    blocks: ChatBlock[];
    origin: string;
    /** Root seq — set to post a thread reply. */
    thread?: number;
  },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      PostMessage: {
        channel_id: params.channelId,
        message_id: params.messageId,
        blocks: params.blocks,
        thread: params.thread ?? null,
        as_agent: null,
      },
    },
    params.origin,
  );

/** Add a reaction (idempotent on the backend — reacting twice with the same
 *  emoji is a no-op there, but the UI should still avoid the redundant submit;
 *  see `hasReacted`). */
export const addReaction = (
  transport: NodeTransport,
  params: { channelId: string; seq: number; emoji: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { AddReaction: { channel_id: params.channelId, seq: params.seq, emoji: params.emoji } },
    params.origin,
  );

export const removeReaction = (
  transport: NodeTransport,
  params: { channelId: string; seq: number; emoji: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { RemoveReaction: { channel_id: params.channelId, seq: params.seq, emoji: params.emoji } },
    params.origin,
  );

/** Replace a message's blocks. Only the stored author may edit (the module
 *  checks the submit origin); `baseRev` records the revision the edit claims to
 *  build on — a stale base is recorded, never rejected (head is last-write-wins
 *  under the consensus order). Like `postMessage`, this sends a single plain
 *  Paragraph; rich blocks/marks are a later increment. */
export const editMessage = (
  transport: NodeTransport,
  params: { channelId: string; seq: number; blocks: ChatBlock[]; baseRev: number | null; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      EditMessage: {
        channel_id: params.channelId,
        seq: params.seq,
        blocks: params.blocks,
        base_rev: params.baseRev,
      },
    },
    params.origin,
  );

/** Tombstone a message: content and reactions are cleared, the skeleton (and
 *  thread linkage) kept. Only the stored author may delete. */
export const deleteMessage = (
  transport: NodeTransport,
  params: { channelId: string; seq: number; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { DeleteMessage: { channel_id: params.channelId, seq: params.seq } },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

export const channels = (transport: NodeTransport): Promise<Channel[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Channels"))
    .then((reply) => replyVariant<Channel[]>(reply, "Channels"));

export const latestMessages = (
  transport: NodeTransport,
  channelId: string,
  limit: number = MAX_QUERY_LIMIT,
): Promise<MessageView[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        MessagesLatest: { channel_id: channelId, limit },
      }),
    )
    .then((reply) => replyVariant<MessageView[]>(reply, "Messages"));

export const thread = (
  transport: NodeTransport,
  params: { channelId: string; rootSeq: number },
): Promise<ChatThread | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        Thread: {
          channel_id: params.channelId,
          root_seq: params.rootSeq,
          from: 0,
          limit: MAX_QUERY_LIMIT,
        },
      }),
    )
    .then((reply) => replyVariant<ChatThread | null>(reply, "Thread"));
