// Optimistic projections — the "preconfirmed render" half of the finalization
// contract (see finalization.ts). Each helper computes the Partial<ConsoleState>
// a write is EXPECTED to produce, applied at submit time so the ui reflects the
// operation before the node confirms it; the next refresh replaces every slice
// with committed truth (and a failed submit refreshes immediately, rolling the
// projection back). Server-derived fields we cannot know yet — heights, owner
// identities, module-assigned ids — are placeholders the refresh corrects.
//
// Everything here is a pure function of (previous state, op params).

import { keyHex } from "../../domain/chat-client";
import type { ChatBlock, HuddleMember, MessageView } from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import type { PageBlock } from "../../domain/pages-client";
import type { ConsoleState } from "./state";

// ── Chat ────────────────────────────────────────────────

/** Map one message (by seq) everywhere it may render: the flat channel list
 *  and the open thread panel's own snapshot. */
const mapMessage = (
  prev: ConsoleState,
  channelId: string,
  seq: number,
  fn: (message: MessageView) => MessageView,
): Partial<ConsoleState> => {
  const touch = (m: MessageView): MessageView =>
    m.channel_id === channelId && m.seq === seq ? fn(m) : m;
  const patch: Partial<ConsoleState> = {
    messages: prev.messages.map(touch),
  };
  if (prev.activeThread) {
    patch.activeThread = {
      root: touch(prev.activeThread.root),
      replies: prev.activeThread.replies.map(touch),
    };
  }
  return patch;
};

export const postedMessage = (
  prev: ConsoleState,
  params: {
    channelId: string;
    messageId: string;
    blocks: ChatBlock[];
    /** The COMMITTED self identity (`selfAuthorBytes`): the node pubkey on a
     *  networked node, the origin bytes on the embedded daemon — so the
     *  optimistic row's author matches what the refresh confirms. */
    authorBytes: number[];
    /** LOCAL wall-clock millis (`Date.now()`) — the same timebase the
     *  embedded daemon commits, so the refresh confirms rather than moves the
     *  stamp there. A height-stamping validator replaces it on refresh, and
     *  the stream builder never day-splits across that timebase seam. */
    atMs: number;
    /** Root seq when this is a thread reply. */
    thread: number | null;
  },
): Partial<ConsoleState> => {
  if (prev.activeChannel !== params.channelId) return {};
  // the seq the single-writer node will MOST LIKELY assign; wrong is fine —
  // the row re-keys to the committed seq on refresh (message_id matches it).
  const seq =
    1 + prev.messages.reduce((max, m) => Math.max(max, m.seq), 0);
  const view: MessageView = {
    channel_id: params.channelId,
    seq,
    head: {
      message_id: params.messageId,
      author: { user: params.authorBytes },
      blocks: params.blocks,
      created_at: params.atMs,
      rev: 0,
      edited_at: null,
      base_rev: null,
      deleted: false,
      thread: params.thread,
      reply_count: 0,
      last_reply_seq: null,
    },
    reactions: [],
    channel_head_seq: seq,
  };
  const patch: Partial<ConsoleState> = {
    messages: [...prev.messages, view],
  };
  if (params.thread !== null && prev.activeThread?.root.seq === params.thread) {
    patch.activeThread = {
      root: {
        ...prev.activeThread.root,
        head: {
          ...prev.activeThread.root.head,
          reply_count: prev.activeThread.root.head.reply_count + 1,
          last_reply_seq: seq,
        },
      },
      replies: [...prev.activeThread.replies, view],
    };
  }
  return patch;
};

export const editedMessage = (
  prev: ConsoleState,
  channelId: string,
  seq: number,
  blocks: ChatBlock[],
  atMs: number,
): Partial<ConsoleState> =>
  mapMessage(prev, channelId, seq, (m) => ({
    ...m,
    head: { ...m.head, blocks, edited_at: atMs, rev: m.head.rev + 1 },
  }));

export const deletedMessage = (
  prev: ConsoleState,
  channelId: string,
  seq: number,
): Partial<ConsoleState> =>
  mapMessage(prev, channelId, seq, (m) => ({
    ...m,
    head: { ...m.head, deleted: true },
  }));

export const reactionToggled = (
  prev: ConsoleState,
  channelId: string,
  seq: number,
  emoji: string,
  selfBytes: number[],
  removing: boolean,
): Partial<ConsoleState> =>
  mapMessage(prev, channelId, seq, (m) => {
    const self = { user: selfBytes };
    const isSelf = (r: MessageView["reactions"][number]["reactors"][number]) =>
      typeof r === "object" &&
      "user" in r &&
      r.user.length === selfBytes.length &&
      r.user.every((b, i) => b === selfBytes[i]);
    const existing = m.reactions.find((r) => r.emoji === emoji);
    const reactions = removing
      ? m.reactions
          .map((r) =>
            r.emoji === emoji
              ? { ...r, reactors: r.reactors.filter((x) => !isSelf(x)) }
              : r,
          )
          .filter((r) => r.reactors.length > 0)
      : existing
        ? m.reactions.map((r) =>
            r.emoji === emoji && !r.reactors.some(isSelf)
              ? { ...r, reactors: [...r.reactors, self] }
              : r,
          )
        : [...m.reactions, { emoji, reactors: [self] }];
    return { ...m, reactions };
  });

export const channelCreated = (
  prev: ConsoleState,
  params: { channelId: string; name: string; postPolicy: PostPolicy; atMs: number },
): Partial<ConsoleState> =>
  prev.channels.some((c) => c.id === params.channelId)
    ? {}
    : {
        channels: [
          ...prev.channels,
          {
            id: params.channelId,
            name: params.name,
            created_at: params.atMs,
            head_seq: 0,
            post_policy: params.postPolicy,
            hooks: [],
            pinned: [],
          },
        ],
      };

/** Add ourselves to a channel's huddle roster the instant we join, so the pill
 *  and dock react before the block lands. Idempotent on our node key; the
 *  refresh replaces the roster (with the module-assigned join order) after. */
export const huddleJoined = (
  prev: ConsoleState,
  params: { channelId: string; node: number[]; authorBytes: number[]; atMs: number },
): Partial<ConsoleState> => {
  const channel = prev.channels.find((c) => c.id === params.channelId);
  if (!channel) return {};
  const selfHex = keyHex(params.node);
  const roster = channel.huddle ?? [];
  if (roster.some((m) => keyHex(m.node) === selfHex)) return {};
  const member: HuddleMember = {
    user: params.authorBytes,
    node: params.node,
    joined_at: params.atMs,
  };
  return {
    channels: prev.channels.map((c) =>
      c.id === params.channelId ? { ...c, huddle: [...roster, member] } : c,
    ),
  };
};

/** Drop our own node from a channel's huddle roster the instant we leave. */
export const huddleLeft = (
  prev: ConsoleState,
  channelId: string,
  selfNodeHex: string,
): Partial<ConsoleState> => ({
  channels: prev.channels.map((c) =>
    c.id === channelId
      ? { ...c, huddle: (c.huddle ?? []).filter((m) => keyHex(m.node) !== selfNodeHex) }
      : c,
  ),
});

/** Drop a swept (stale) member from a channel's huddle roster the instant the
 *  sweep is submitted — keyed by the target's submitter identity (user) hex,
 *  the same key the sweep op carries (unlike leave, which keys on the mesh
 *  node). The refresh replaces the roster with committed truth after. */
export const huddleSwept = (
  prev: ConsoleState,
  channelId: string,
  userKeyHex: string,
): Partial<ConsoleState> => ({
  channels: prev.channels.map((c) =>
    c.id === channelId
      ? { ...c, huddle: (c.huddle ?? []).filter((m) => keyHex(m.user) !== userKeyHex) }
      : c,
  ),
});

// ── Pages ───────────────────────────────────────────────

export const pageCreated = (
  prev: ConsoleState,
  params: { pageId: string; title: string; parent?: string | null },
): Partial<ConsoleState> => ({
  pages: [
    ...prev.pages,
    { id: params.pageId, title: params.title, parent: params.parent ?? null },
  ],
});

/** Every id in `blockId`'s subtree (itself included), via the children links. */
const subtreeIds = (blocks: PageBlock[], blockId: string): Set<string> => {
  const byId = new Map(blocks.map((b) => [b.id, b]));
  const ids = new Set<string>();
  const walk = (id: string) => {
    if (ids.has(id)) return;
    ids.add(id);
    byId.get(id)?.children.forEach(walk);
  };
  walk(blockId);
  return ids;
};

/** The flat-list index just past `blockId`'s whole subtree — where a next
 *  sibling lands in preorder. */
const afterSubtreeIndex = (blocks: PageBlock[], blockId: string): number => {
  const ids = subtreeIds(blocks, blockId);
  let last = -1;
  blocks.forEach((b, i) => {
    if (ids.has(b.id)) last = i;
  });
  return last + 1;
};

/** The flat-list index where a block joins `parent` at the `after` anchor:
 *  a first child sits right after the parent; a sibling anchor puts it past
 *  that sibling's whole subtree. Null when the parent or anchor is missing
 *  (a torn snapshot) — the caller defers to the refresh. */
const preorderIndex = (
  blocks: PageBlock[],
  parent: string,
  after: string | null,
): number | null => {
  const parentAt = blocks.findIndex((b) => b.id === parent);
  if (parentAt === -1) return null;
  if (after === null) return parentAt + 1;
  return blocks.some((b) => b.id === after)
    ? afterSubtreeIndex(blocks, after)
    : null;
};

/** `children` with `id` linked in right after `after` (first when null). */
const linkChild = (children: string[], id: string, after: string | null): string[] => {
  const at = after === null ? 0 : children.indexOf(after) + 1;
  return [...children.slice(0, at), id, ...children.slice(at)];
};

export const pageBlockInserted = (
  prev: ConsoleState,
  params: { parent: string; after: string | null; block: PageBlock },
): Partial<ConsoleState> => {
  const blocks = prev.activePageBlocks;
  const at = preorderIndex(blocks, params.parent, params.after);
  if (at === null) return {}; // torn snapshot — let the refresh place it
  const linked = blocks.map((b) =>
    b.id === params.parent
      ? { ...b, children: linkChild(b.children, params.block.id, params.after) }
      : b,
  );
  return {
    activePageBlocks: [...linked.slice(0, at), params.block, ...linked.slice(at)],
  };
};

/** Re-home `blockId`'s whole subtree under `parent` at the `after` anchor —
 *  the projection behind indent/outdent, alt-arrows, drag-drop, and the
 *  merge's child adoption. The subtree's flat rows lift out in document order
 *  and re-splice at the target's preorder position; the old and new parents'
 *  children links (and the block's own parent) are patched to match. A target
 *  inside the moving subtree is a cycle the module would reject — never
 *  render it; a missing block, parent, or anchor is a torn snapshot. All
 *  defer to the refresh. */
export const pageBlockMoved = (
  prev: ConsoleState,
  params: { blockId: string; parent: string; after: string | null },
): Partial<ConsoleState> => {
  const blocks = prev.activePageBlocks;
  const block = blocks.find((b) => b.id === params.blockId);
  if (!block) return {};
  const moving = subtreeIds(blocks, params.blockId);
  if (moving.has(params.parent)) return {};
  if (params.after !== null && moving.has(params.after)) return {};
  // lift the subtree out and unlink it from its old parent, THEN place: the
  // anchor index and the new children link are both computed against the
  // lifted list, so a same-parent reorder needs no special case.
  const rest = blocks
    .filter((b) => !moving.has(b.id))
    .map((b) =>
      b.id === block.parent
        ? { ...b, children: b.children.filter((c) => c !== params.blockId) }
        : b,
    );
  const at = preorderIndex(rest, params.parent, params.after);
  if (at === null) return {};
  const linked = rest.map((b) =>
    b.id === params.parent
      ? { ...b, children: linkChild(b.children, params.blockId, params.after) }
      : b,
  );
  const subtree = blocks
    .filter((b) => moving.has(b.id))
    .map((b) => (b.id === params.blockId ? { ...b, parent: params.parent } : b));
  return {
    activePageBlocks: [...linked.slice(0, at), ...subtree, ...linked.slice(at)],
  };
};

/** Patch one page block in place; renaming a page ROOT also renames the page
 *  in the rail enumeration (title == root text). */
export const pageBlockPatched = (
  prev: ConsoleState,
  blockId: string,
  patch: Partial<Pick<PageBlock, "text" | "kind" | "checked">>,
): Partial<ConsoleState> => {
  const target = prev.activePageBlocks.find((b) => b.id === blockId);
  const out: Partial<ConsoleState> = {
    activePageBlocks: prev.activePageBlocks.map((b) =>
      b.id === blockId ? { ...b, ...patch } : b,
    ),
  };
  if (target && target.parent === null && typeof patch.text === "string") {
    out.pages = prev.pages.map((p) =>
      p.id === target.id ? { ...p, title: patch.text as string } : p,
    );
  }
  return out;
};

export const pageBlockRemoved = (
  prev: ConsoleState,
  blockId: string,
): Partial<ConsoleState> => {
  const gone = subtreeIds(prev.activePageBlocks, blockId);
  return {
    activePageBlocks: prev.activePageBlocks
      .filter((b) => !gone.has(b.id))
      .map((b) =>
        b.children.includes(blockId)
          ? { ...b, children: b.children.filter((c) => c !== blockId) }
          : b,
      ),
  };
};

// ── Agents ──────────────────────────────────────────────

export const agentPatched = (
  prev: ConsoleState,
  agentId: string,
  patch: Partial<Pick<ConsoleState["agents"][number], "status" | "display_name" | "capability">>,
): Partial<ConsoleState> => ({
  agents: prev.agents.map((a) =>
    a.agent_id === agentId ? { ...a, ...patch } : a,
  ),
});

export const watchSet = (
  prev: ConsoleState,
  params: { channelId: string; policy: ConsoleState["watches"][number]["policy"] },
): Partial<ConsoleState> => ({
  watches: [
    ...prev.watches.filter((w) => w.channel_id !== params.channelId),
    { channel_id: params.channelId, policy: params.policy },
  ],
});

export const watchRemoved = (
  prev: ConsoleState,
  channelId: string,
): Partial<ConsoleState> => ({
  watches: prev.watches.filter((w) => w.channel_id !== channelId),
});

export const runCancelled = (
  prev: ConsoleState,
  runId: string,
): Partial<ConsoleState> => ({
  // a cancel resolves through the dispatch plane's Err("cancelled") delivery,
  // which prunes the entry node-side a block later — mirror that prune.
  pendingRuns: prev.pendingRuns.filter((r) => r.run_id !== runId),
});
