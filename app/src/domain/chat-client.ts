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
  | { user: number[] }
  | { agent: { module: string; agent_id: string } }
  | { module: string }
  | "system";

export type Mark = "bold" | "italic" | { link: string } | { mention: AuthorRef };

export interface Span {
  text: string;
  marks: Mark[];
}

export type ChatBlock =
  | { paragraph: Span[] }
  | { code: { lang: string | null; text: string } }
  | { quote: Span[] }
  | "divider";

export type PostPolicy = "open" | "members_only";

/** One participant in a channel's voice huddle, in join order. `user` is the
 *  submitter identity bytes (the AuthorRef::User bytes — a readable origin name
 *  on the embedded daemon), `node` is that member's 32-byte ed25519 mesh key
 *  (the voice fan-out address), `joined_at` the consensus time it joined. */
export interface HuddleMember {
  user: number[];
  node: number[];
  joined_at: number;
}

export interface Channel {
  id: string;
  name: string;
  created_at: number;
  head_seq: number;
  post_policy: PostPolicy;
  hooks: string[];
  pinned: number[];
  /** Live voice huddle roster, in join order. Empty/absent = no huddle. */
  huddle?: HuddleMember[];
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

/** Inverse of `keyHex` — a hex key string back to its raw bytes. Used to turn
 *  status.publicKey (64-char hex mesh identity) into the `node` byte array a
 *  join_huddle op carries. One converter for the whole domain layer: this is
 *  agent-client's `hexToBytes` under the roster's vocabulary. */
export { hexToBytes as keyBytes } from "./agent-client";

/** A display name for an author. A User author's bytes are the submitter
 *  identity the daemon stamped; when the `profiles` registry (`names`) resolves
 *  those bytes it wins, else we fall back to the utf-8/hex handle. */
export const authorName = (author: AuthorRef, names?: AuthorNames): string => {
  if (author === "system") return "system";
  if ("user" in author)
    return names?.[keyHex(author.user)] ?? displayUserBytes(author.user);
  if ("agent" in author) return `${author.agent.module}/${author.agent.agent_id}`;
  return author.module;
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
      if (block === "divider") return "———";
      if ("paragraph" in block) return spanText(block.paragraph);
      if ("quote" in block) return `> ${spanText(block.quote)}`;
      return block.code.text;
    })
    .join("\n");

// ── Msgs (writes — one submit = one block) ──────────────

export const createChannel = (
  transport: NodeTransport,
  params: { channelId: string; name: string; postPolicy: PostPolicy; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    {
      create_channel: {
        channel_id: params.channelId,
        name: params.name,
        post_policy: params.postPolicy,
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
      post_message: {
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
    { add_reaction: { channel_id: params.channelId, seq: params.seq, emoji: params.emoji } },
    params.origin,
  );

export const removeReaction = (
  transport: NodeTransport,
  params: { channelId: string; seq: number; emoji: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { remove_reaction: { channel_id: params.channelId, seq: params.seq, emoji: params.emoji } },
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
      edit_message: {
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
    { delete_message: { channel_id: params.channelId, seq: params.seq } },
    params.origin,
  );

// ── Huddle (voice roster ops — consensus membership, not the audio) ──

/** Join a channel's voice huddle. `node` is THIS node's 32-byte ed25519 mesh
 *  key (status.publicKey decoded); the module gates members-only channels like
 *  posting and is idempotent on a re-join. Authorship comes from `origin`. */
export const joinHuddle = (
  transport: NodeTransport,
  params: { channelId: string; node: number[]; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { join_huddle: { channel_id: params.channelId, node: params.node } },
    params.origin,
  );

/** Leave a channel's voice huddle (idempotent — leaving twice is a no-op). */
export const leaveHuddle = (
  transport: NodeTransport,
  params: { channelId: string; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { leave_huddle: { channel_id: params.channelId } },
    params.origin,
  );

/** Evict a (stale) huddle member — consensus cleanup for a client that died
 *  without leaving (its beacons went silent). Keyed by the member's submitter
 *  identity bytes (`user`), not its mesh node key; the module gates it
 *  members-only like posting. Authorship comes from `origin`. */
export const sweepHuddle = (
  transport: NodeTransport,
  params: { channelId: string; user: number[]; origin: string },
): Promise<BlockEvent> =>
  transport.submit(
    TARGET,
    { sweep_huddle: { channel_id: params.channelId, user: params.user } },
    params.origin,
  );

// ── Queries (reads over committed state) ────────────────

export const channels = (transport: NodeTransport): Promise<Channel[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "channels"))
    .then((reply) => replyVariant<Channel[]>(reply, "channels"));

export const latestMessages = (
  transport: NodeTransport,
  channelId: string,
  limit: number = MAX_QUERY_LIMIT,
): Promise<MessageView[]> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        messages_latest: { channel_id: channelId, limit },
      }),
    )
    .then((reply) => replyVariant<MessageView[]>(reply, "messages"));

export const thread = (
  transport: NodeTransport,
  params: { channelId: string; rootSeq: number },
): Promise<ChatThread | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        thread: {
          channel_id: params.channelId,
          root_seq: params.rootSeq,
          from: 0,
          limit: MAX_QUERY_LIMIT,
        },
      }),
    )
    .then((reply) => replyVariant<ChatThread | null>(reply, "thread"));

// ── Materialized view (the module's derived-index endpoint) ──

/** One search hit from chat's materialized view. camelCase wire — the
 *  `*-index` tier's convention, unlike the module's snake_case canonical
 *  shapes above. `author` is pre-rendered by the index ("user:jess",
 *  "agent:agent/helper", "module:automations", "system"). */
export interface ChatSearchHit {
  channelId: string;
  seq: number;
  messageId: string;
  author: string;
  height: number;
  time: number;
  text: string;
  deleted: boolean;
  edited: boolean;
  thread?: number;
  /** Normalized #tag labels the head carries (absent when untagged). */
  tags?: string[];
}

/** Full-text search over message heads, newest first — served by the node's
 *  per-module index (chat's own view endpoint), not canonical state. */
export const searchMessages = (
  transport: NodeTransport,
  params: { text: string; channelId?: string; limit?: number },
): Promise<ChatSearchHit[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, {
        search: { text: params.text, channelId: params.channelId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<ChatSearchHit[]>(reply, "hits"));

/** One row of the tag catalog: a normalized label, how many live messages
 *  carry it in the asked scope, and the newest such message's seq. */
export interface ChatTagRow {
  tag: string;
  count: number;
  lastSeq: number;
}

/** The tag catalog — a channel's live #tags ordered by count desc then tag
 *  asc (no channelId aggregates the whole workspace). Served by the same
 *  node-local derived index as `searchMessages`. */
export const tags = (
  transport: NodeTransport,
  params: { channelId?: string; limit?: number } = {},
): Promise<ChatTagRow[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, {
        tags: { channelId: params.channelId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<ChatTagRow[]>(reply, "tags"));

/** Every live message carrying one exact #tag, newest first. The node
 *  normalizes the queried tag (NFC + lowercase, leading `#` stripped), so an
 *  as-typed display form can be passed straight through. */
export const tagSearch = (
  transport: NodeTransport,
  params: { tag: string; channelId?: string; limit?: number },
): Promise<ChatSearchHit[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, {
        tagSearch: { tag: params.tag, channelId: params.channelId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<ChatSearchHit[]>(reply, "hits"));
