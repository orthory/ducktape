// Typed client for the node's `pages` module — the TS mirror of
// `crates/apps/pages-interface`. A page is a TREE of blocks: the page itself
// is the root block (kind "page", text == title), each block carries an
// ordered `children` list, and block ids are GLOBALLY UNIQUE within the
// module — getBlock resolves a bare id with no page context, which is what
// makes a block referenceable from elsewhere (see BlockRef). Same contract as
// document-client/tasks-client: camelCase params in, verbatim serde wire out,
// pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

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

/** One entry of the page enumeration: id + live title. */
export interface PageMeta {
  id: string;
  title: string;
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
  params: { pageId: string; title: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    create_page: { page_id: params.pageId, title: params.title },
  });

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
