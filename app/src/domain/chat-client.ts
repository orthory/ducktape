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

/** A display name for an author. A User author's bytes are the submitter
 *  identity the daemon stamped — for this app, the utf-8 display name. */
export const authorName = (author: AuthorRef): string => {
  if (author === "System") return "system";
  if ("User" in author) return new TextDecoder().decode(new Uint8Array(author.User));
  if ("Agent" in author) return `${author.Agent.module}/${author.Agent.agent_id}`;
  return author.Module;
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
    text: string;
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
        blocks: [{ Paragraph: [{ text: params.text, marks: [] }] }],
        thread: params.thread ?? null,
        as_agent: null,
      },
    },
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
