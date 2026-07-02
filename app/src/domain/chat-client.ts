// Typed client for the node's `chat` module — the TS mirror of
// `crates/apps/chat-interface`.
//
// This file plays the interface crate's role on the client side: it is the
// only place that knows chat's wire shape. Functions take camelCase params and
// encode them to the exact json serde produces for `ChatMsg` / `ChatQuery`
// (PascalCase variants, snake_case fields); replies decode `ChatReply` the
// same way. Everything is a pure function over an injected NodeTransport —
// no module-level transport state.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (ChatReply payloads, verbatim) ───────────

export interface ChatChannel {
  id: string;
  name: string;
  created_at: number;
}

export interface ChatMessage {
  id: string;
  channel_id: string;
  author: string;
  body: string;
  sequence: number;
  sent_at: number;
  thread_id: string | null;
  reply_count: number;
  last_reply_at: number | null;
}

export interface ChatThread {
  root: ChatMessage;
  replies: ChatMessage[];
}

const TARGET = "chat";

// ── Msgs (writes — one submit = one block) ──────────────

export const createChannel = (
  transport: NodeTransport,
  params: { channelId: string; name: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    CreateChannel: { channel_id: params.channelId, name: params.name },
  });

export const sendMessage = (
  transport: NodeTransport,
  params: { channelId: string; messageId: string; author: string; body: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    SendMessage: {
      channel_id: params.channelId,
      message_id: params.messageId,
      author: params.author,
      body: params.body,
    },
  });

export const replyInThread = (
  transport: NodeTransport,
  params: {
    channelId: string;
    threadId: string;
    messageId: string;
    author: string;
    body: string;
  },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    ReplyInThread: {
      channel_id: params.channelId,
      thread_id: params.threadId,
      message_id: params.messageId,
      author: params.author,
      body: params.body,
    },
  });

// ── Queries (reads over committed state) ────────────────

export const channels = (transport: NodeTransport): Promise<ChatChannel[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "Channels"))
    .then((reply) => replyVariant<ChatChannel[]>(reply, "Channels"));

export const messages = (
  transport: NodeTransport,
  channelId: string,
): Promise<ChatMessage[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, { Messages: { channel_id: channelId } }),
    )
    .then((reply) => replyVariant<ChatMessage[]>(reply, "Messages"));

export const thread = (
  transport: NodeTransport,
  params: { channelId: string; threadId: string },
): Promise<ChatThread | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        Thread: { channel_id: params.channelId, thread_id: params.threadId },
      }),
    )
    .then((reply) => replyVariant<ChatThread | null>(reply, "Thread"));
