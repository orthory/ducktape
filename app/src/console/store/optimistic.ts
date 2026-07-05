// Optimistic projections — the "preconfirmed render" half of the finalization
// contract (see finalization.ts). Each helper computes the Partial<ConsoleState>
// a write is EXPECTED to produce, applied at submit time so the ui reflects the
// operation before the node confirms it; the next refresh replaces every slice
// with committed truth (and a failed submit refreshes immediately, rolling the
// projection back). Server-derived fields we cannot know yet — heights, owner
// identities, module-assigned ids — are placeholders the refresh corrects.
//
// Everything here is a pure function of (previous state, op params).

import type { ChatBlock, MessageView } from "../../domain/chat-client";
import type { PostPolicy } from "../../domain/chat-client";
import type { Block } from "../../domain/document-client";
import type { Job } from "../../domain/jobs-client";
import type { LsEntry, Meta } from "../../domain/memory-client";
import type { PageBlock } from "../../domain/pages-client";
import type { Rule } from "../../domain/automations-client";
import type { TaskStatus } from "../../domain/tasks-client";
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
    author: string;
    at: number;
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
      author: { User: Array.from(new TextEncoder().encode(params.author)) },
      blocks: params.blocks,
      created_at: params.at,
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
  at: number,
): Partial<ConsoleState> =>
  mapMessage(prev, channelId, seq, (m) => ({
    ...m,
    head: { ...m.head, blocks, edited_at: at, rev: m.head.rev + 1 },
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
    const self = { User: selfBytes };
    const isSelf = (r: MessageView["reactions"][number]["reactors"][number]) =>
      typeof r === "object" &&
      "User" in r &&
      r.User.length === selfBytes.length &&
      r.User.every((b, i) => b === selfBytes[i]);
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
  params: { channelId: string; name: string; postPolicy: PostPolicy; at: number },
): Partial<ConsoleState> =>
  prev.channels.some((c) => c.id === params.channelId)
    ? {}
    : {
        channels: [
          ...prev.channels,
          {
            id: params.channelId,
            name: params.name,
            created_at: params.at,
            head_seq: 0,
            post_policy: params.postPolicy,
            hooks: [],
            pinned: [],
          },
        ],
      };

// ── Tasks ───────────────────────────────────────────────

export const taskAdded = (
  prev: ConsoleState,
  params: { taskId: string; title: string; at: number },
): Partial<ConsoleState> => ({
  tasks: [
    ...prev.tasks,
    {
      id: params.taskId,
      title: params.title,
      status: "Open",
      created_at: params.at,
      updated_at: params.at,
    },
  ],
});

export const taskAdvanced = (
  prev: ConsoleState,
  taskId: string,
  status: TaskStatus,
): Partial<ConsoleState> => ({
  tasks: prev.tasks.map((t) => (t.id === taskId ? { ...t, status } : t)),
});

// ── Documents ───────────────────────────────────────────

export const docCreated = (
  prev: ConsoleState,
  docId: string,
): Partial<ConsoleState> =>
  prev.docIds.includes(docId) ? {} : { docIds: [...prev.docIds, docId].sort() };

/** `after` rule (InsertBlock/MoveBlock): null == front, an id == right after
 *  that block. An unknown anchor appends — the refresh corrects it. */
const docInsertIndex = (blocks: Block[], after: string | null): number => {
  if (after === null) return 0;
  const at = blocks.findIndex((b) => b.id === after);
  return at === -1 ? blocks.length : at + 1;
};

export const docBlockInserted = (
  prev: ConsoleState,
  params: { after: string | null; block: Block },
): Partial<ConsoleState> => {
  const blocks = [...prev.activeDocBlocks];
  blocks.splice(docInsertIndex(blocks, params.after), 0, params.block);
  return { activeDocBlocks: blocks };
};

export const docBlockUpdated = (
  prev: ConsoleState,
  blockId: string,
  text: string,
): Partial<ConsoleState> => ({
  activeDocBlocks: prev.activeDocBlocks.map((b) =>
    b.id === blockId ? { ...b, text } : b,
  ),
});

export const docBlockRemoved = (
  prev: ConsoleState,
  blockId: string,
): Partial<ConsoleState> => ({
  activeDocBlocks: prev.activeDocBlocks.filter((b) => b.id !== blockId),
});

export const docBlockMoved = (
  prev: ConsoleState,
  params: { blockId: string; after: string | null },
): Partial<ConsoleState> => {
  const moving = prev.activeDocBlocks.find((b) => b.id === params.blockId);
  if (!moving) return {};
  const rest = prev.activeDocBlocks.filter((b) => b.id !== params.blockId);
  rest.splice(docInsertIndex(rest, params.after), 0, moving);
  return { activeDocBlocks: rest };
};

// ── Pages ───────────────────────────────────────────────

export const pageCreated = (
  prev: ConsoleState,
  params: { pageId: string; title: string },
): Partial<ConsoleState> => ({
  pages: [...prev.pages, { id: params.pageId, title: params.title }],
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

export const pageBlockInserted = (
  prev: ConsoleState,
  params: { parent: string; after: string | null; block: PageBlock },
): Partial<ConsoleState> => {
  const blocks = prev.activePageBlocks;
  const parentAt = blocks.findIndex((b) => b.id === params.parent);
  if (parentAt === -1) return {}; // torn snapshot — let the refresh place it
  // preorder position: first child sits right after the parent; a sibling
  // anchor puts us past that sibling's whole subtree.
  const at =
    params.after === null
      ? parentAt + 1
      : afterSubtreeIndex(blocks, params.after);
  const next = [...blocks];
  next.splice(at, 0, params.block);
  return {
    activePageBlocks: next.map((b) => {
      if (b.id !== params.parent) return b;
      const children = [...b.children];
      const childAt =
        params.after === null ? 0 : children.indexOf(params.after) + 1;
      children.splice(childAt, 0, params.block.id);
      return { ...b, children };
    }),
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

// ── Inbox ───────────────────────────────────────────────

export const inboxReadTo = (
  prev: ConsoleState,
  upToSeq: number,
): Partial<ConsoleState> => {
  const inbox = prev.inbox.map((n) =>
    n.seq <= upToSeq ? { ...n, read: true } : n,
  );
  return { inbox, inboxUnread: inbox.filter((n) => !n.read).length };
};

export const inboxCleared = (
  prev: ConsoleState,
  upToSeq: number,
): Partial<ConsoleState> => {
  const inbox = prev.inbox.filter((n) => n.seq > upToSeq);
  return { inbox, inboxUnread: inbox.filter((n) => !n.read).length };
};

// ── Jobs ────────────────────────────────────────────────

export const jobAdded = (prev: ConsoleState, job: Job): Partial<ConsoleState> => ({
  jobs: [...prev.jobs, job],
});

export const jobPatched = (
  prev: ConsoleState,
  jobId: string,
  patch: Partial<Job>,
): Partial<ConsoleState> => ({
  jobs: prev.jobs.map((j) => (j.job_id === jobId ? { ...j, ...patch } : j)),
});

export const jobRemoved = (
  prev: ConsoleState,
  jobId: string,
): Partial<ConsoleState> => ({
  jobs: prev.jobs.filter((j) => j.job_id !== jobId),
});

// ── Automations ─────────────────────────────────────────

export const ruleAdded = (prev: ConsoleState, rule: Rule): Partial<ConsoleState> => ({
  rules: [...prev.rules, rule],
});

export const rulePatched = (
  prev: ConsoleState,
  ruleId: string,
  patch: Partial<Rule>,
): Partial<ConsoleState> => ({
  rules: prev.rules.map((r) => (r.rule_id === ruleId ? { ...r, ...patch } : r)),
});

export const ruleRemoved = (
  prev: ConsoleState,
  ruleId: string,
): Partial<ConsoleState> => ({
  rules: prev.rules.filter((r) => r.rule_id !== ruleId),
});

// ── Memory ──────────────────────────────────────────────

const memoryParent = (path: string): string => {
  const cut = path.lastIndexOf("/");
  return cut <= 0 ? "/" : path.slice(0, cut);
};

/** Upsert the published file into the OPEN directory listing (a publish into
 *  some other dir shows up when that dir is browsed — server truth anyway). */
export const memoryPublished = (
  prev: ConsoleState,
  params: { path: string; bodyLen: number; meta?: Meta },
): Partial<ConsoleState> => {
  if (memoryParent(params.path) !== prev.memoryPath) return {};
  const existing = prev.memoryEntries.find(
    (e) => "File" in e && e.File.path === params.path,
  );
  const stat = (gen: number, gens: number): LsEntry => ({
    File: {
      path: params.path,
      latest_generation: gen,
      generations: gens,
      latest_meta: params.meta ?? {},
      latest_author: "", // origin-derived server-side; refresh fills it
      latest_published_at_height: 0,
      body_len: params.bodyLen,
    },
  });
  return {
    memoryEntries: existing
      ? prev.memoryEntries.map((e) =>
          "File" in e && e.File.path === params.path
            ? stat(e.File.latest_generation + 1, e.File.generations + 1)
            : e,
        )
      : [...prev.memoryEntries, stat(1, 1)],
  };
};

export const memoryRemoved = (
  prev: ConsoleState,
  path: string,
): Partial<ConsoleState> => ({
  memoryEntries: prev.memoryEntries.filter(
    (e) => !("File" in e) || e.File.path !== path,
  ),
});

// ── Files ───────────────────────────────────────────────

export const fileRemoved = (
  prev: ConsoleState,
  fileId: string,
): Partial<ConsoleState> => ({
  files: prev.files.filter((m) => m.file_id !== fileId),
});
