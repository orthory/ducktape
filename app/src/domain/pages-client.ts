// Typed client for the node's `pages` module — the TS mirror of
// `crates/apps/pages-interface`. A page is a TREE of blocks: the page itself
// is the root block (kind "page", text == title), each block carries an
// ordered `children` list, and block ids are GLOBALLY UNIQUE within the
// module — getBlock resolves a bare id with no page context, which is what
// makes a block referenceable from elsewhere (see BlockRef). Same contract as
// document-client/tasks-client: camelCase params in, verbatim serde wire out,
// pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import type { AuthorRef } from "./chat-client";
import { replyVariant } from "./wire";

export type { AuthorRef };

// ── Wire types (block records + PageReply payloads, verbatim) ─

export type BlockKind =
  | "page"
  | "paragraph"
  | "heading1"
  | "heading2"
  | "heading3"
  | "bulleted"
  | "numbered"
  | "todo"
  | "toggle"
  | "quote"
  | "code"
  | "callout"
  | "divider";

/** One stored block. `parent` is null only for a page root; `page` names the
 *  root block of the page this block belongs to (a root names itself);
 *  `checked` is only meaningful for kind "todo". */
export interface PageBlock {
  id: string;
  parent: string | null;
  page: string;
  kind: BlockKind;
  text: string;
  checked: boolean;
  children: string[];
}

/** One entry of the page enumeration: id + live title + folder parent. */
export interface PageMeta {
  id: string;
  title: string;
  /** Folder parent page id, or null for a top-level page. */
  parent: string | null;
}

/** A stable pointer to one block in one pages module — the shape a future
 *  cross-module reference carries. Resolvable today via getBlock(block). */
export interface BlockRef {
  module: string;
  block: string;
}

const TARGET = "pages";

// ── Msgs (writes — one block op per submit; no origin) ──
//
// `after` positioning (same rule for InsertBlock and MoveBlock): null ==
// "first child of parent"; a block id == "immediately after that sibling"
// (the anchor must be a child of parent, else the op errors).

export const createPage = (
  transport: NodeTransport,
  params: { pageId: string; title: string; parent?: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    create_page: {
      page_id: params.pageId,
      title: params.title,
      parent: params.parent ?? null,
    },
  });

/** Re-nest a page under a (possibly new) parent page, or to top level with
 *  null. */
export const setPageParent = (
  transport: NodeTransport,
  params: { pageId: string; parent: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    set_page_parent: { page_id: params.pageId, parent: params.parent },
  });

/** Delete a page: its root + block subtree; child pages are promoted up. */
export const deletePage = (
  transport: NodeTransport,
  pageId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { delete_page: { page_id: pageId } });

export const insertBlock = (
  transport: NodeTransport,
  params: {
    parent: string;
    after: string | null;
    block: { id: string; kind: BlockKind; text: string };
  },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    insert_block: {
      parent: params.parent,
      after: params.after,
      block: params.block,
    },
  });

export const updateText = (
  transport: NodeTransport,
  params: { blockId: string; text: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    update_text: { block_id: params.blockId, text: params.text },
  });

export const setKind = (
  transport: NodeTransport,
  params: { blockId: string; kind: BlockKind },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    set_kind: { block_id: params.blockId, kind: params.kind },
  });

export const setChecked = (
  transport: NodeTransport,
  params: { blockId: string; checked: boolean },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    set_checked: { block_id: params.blockId, checked: params.checked },
  });

export const moveBlock = (
  transport: NodeTransport,
  params: { blockId: string; parent: string; after: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    move_block: {
      block_id: params.blockId,
      parent: params.parent,
      after: params.after,
    },
  });

export const removeBlock = (
  transport: NodeTransport,
  blockId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { remove_block: { block_id: blockId } });

// ── Queries (reads over committed state) ────────────────

/** The whole page in PREORDER (root first, each subtree before its next
 *  sibling), or null when no page lives at that id. */
export const getPage = (
  transport: NodeTransport,
  pageId: string,
): Promise<PageBlock[] | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { get_page: { page_id: pageId } }))
    .then((reply) => replyVariant<PageBlock[] | null>(reply, "page"));

/** A single block by id ALONE — the BlockRef resolution surface. The reply
 *  carries the block's `page` and `parent`, so a resolver learns where the
 *  block lives, not just what it says. */
export const getBlock = (
  transport: NodeTransport,
  blockId: string,
): Promise<PageBlock | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { get_block: { block_id: blockId } }))
    .then((reply) => replyVariant<PageBlock | null>(reply, "block"));

/** Every page, sorted by id, with live titles — the module's enumeration
 *  index joined against the root blocks. */
export const listPages = (transport: NodeTransport): Promise<PageMeta[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "list_pages"))
    .then((reply) => replyVariant<PageMeta[]>(reply, "page_list"));

// ── Materialized view (the module's derived-index endpoint) ──

/** One search hit from pages' materialized view (camelCase index wire). */
export interface PageSearchHit {
  blockId: string;
  pageId: string;
  parent?: string;
  kind: BlockKind;
  text: string;
  height: number;
  time: number;
}

/** Full-text search over the page block tree, newest first — served by the
 *  node's per-module index (pages' own view endpoint), not canonical state. */
export const searchPageBlocks = (
  transport: NodeTransport,
  params: { text: string; pageId?: string; limit?: number },
): Promise<PageSearchHit[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, {
        search: { text: params.text, pageId: params.pageId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<PageSearchHit[]>(reply, "hits"));

// ── Comments (threads anchored to a block/page, in the same module) ─────
//
// A comment thread anchors to a `target` — a block id or a page id in THIS
// module. Authorship is derived by the module from the submit origin, so it
// appears only in replies.

export interface Comment {
  id: string;
  thread_id: string;
  author: AuthorRef;
  text: string;
  created_at: number;
  edited_at: number | null;
  deleted: boolean;
}

export interface Thread {
  id: string;
  target: string;
  opener: AuthorRef;
  created_at: number;
  resolved: boolean;
  resolved_by: AuthorRef | null;
  comment_ids: string[];
}

export interface ThreadView {
  thread: Thread;
  comments: Comment[];
}

export interface TargetThreads {
  target: string;
  threads: ThreadView[];
}

export const addComment = (
  transport: NodeTransport,
  params: {
    threadId: string;
    commentId: string;
    target: string;
    text: string;
    mentions?: AuthorRef[];
  },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    add_comment: {
      thread_id: params.threadId,
      comment_id: params.commentId,
      target: params.target,
      text: params.text,
      mentions: params.mentions ?? [],
    },
  });

export const editComment = (
  transport: NodeTransport,
  params: { commentId: string; text: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    edit_comment: { comment_id: params.commentId, text: params.text },
  });

export const deleteComment = (
  transport: NodeTransport,
  commentId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { delete_comment: { comment_id: commentId } });

export const resolveThread = (
  transport: NodeTransport,
  params: { threadId: string; resolved: boolean },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    resolve_thread: { thread_id: params.threadId, resolved: params.resolved },
  });

/** Every thread anchored to any of `targets` (block/page ids), grouped by
 *  target — one round-trip for a whole page's visible blocks. */
export const threadsForTargets = (
  transport: NodeTransport,
  params: { targets: string[] },
): Promise<TargetThreads[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { threads_for_targets: { targets: params.targets } }))
    .then((reply) => replyVariant<TargetThreads[]>(reply, "comment_threads"));

export const getThread = (
  transport: NodeTransport,
  threadId: string,
): Promise<ThreadView | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { comment_thread: { thread_id: threadId } }))
    .then((reply) => replyVariant<ThreadView | null>(reply, "comment_thread"));
