// Typed client for the node's `pages` module — the TS mirror of
// `crates/apps/pages-interface`. A page is a TREE of blocks: the page itself
// is the root block (kind "Page", text == title), each block carries an
// ordered `children` list, and block ids are GLOBALLY UNIQUE within the
// module — getBlock resolves a bare id with no page context, which is what
// makes a block referenceable from elsewhere (see BlockRef). Same contract as
// document-client/tasks-client: camelCase params in, verbatim serde wire out,
// pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (block records + PageReply payloads, verbatim) ─

export type BlockKind =
  | "Page"
  | "Paragraph"
  | "Heading1"
  | "Heading2"
  | "Heading3"
  | "Bulleted"
  | "Numbered"
  | "Todo"
  | "Toggle"
  | "Quote"
  | "Code"
  | "Callout"
  | "Divider";

/** One stored block. `parent` is null only for a page root; `page` names the
 *  root block of the page this block belongs to (a root names itself);
 *  `checked` is only meaningful for kind "Todo". */
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
    CreatePage: { page_id: params.pageId, title: params.title },
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
    InsertBlock: {
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
    UpdateText: { block_id: params.blockId, text: params.text },
  });

export const setKind = (
  transport: NodeTransport,
  params: { blockId: string; kind: BlockKind },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    SetKind: { block_id: params.blockId, kind: params.kind },
  });

export const setChecked = (
  transport: NodeTransport,
  params: { blockId: string; checked: boolean },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    SetChecked: { block_id: params.blockId, checked: params.checked },
  });

export const moveBlock = (
  transport: NodeTransport,
  params: { blockId: string; parent: string; after: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    MoveBlock: {
      block_id: params.blockId,
      parent: params.parent,
      after: params.after,
    },
  });

export const removeBlock = (
  transport: NodeTransport,
  blockId: string,
): Promise<BlockEvent> =>
  transport.submit(TARGET, { RemoveBlock: { block_id: blockId } });

// ── Queries (reads over committed state) ────────────────

/** The whole page in PREORDER (root first, each subtree before its next
 *  sibling), or null when no page lives at that id. */
export const getPage = (
  transport: NodeTransport,
  pageId: string,
): Promise<PageBlock[] | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { GetPage: { page_id: pageId } }))
    .then((reply) => replyVariant<PageBlock[] | null>(reply, "Page"));

/** A single block by id ALONE — the BlockRef resolution surface. The reply
 *  carries the block's `page` and `parent`, so a resolver learns where the
 *  block lives, not just what it says. */
export const getBlock = (
  transport: NodeTransport,
  blockId: string,
): Promise<PageBlock | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { GetBlock: { block_id: blockId } }))
    .then((reply) => replyVariant<PageBlock | null>(reply, "Block"));

/** Every page, sorted by id, with live titles — the module's enumeration
 *  index joined against the root blocks. */
export const listPages = (transport: NodeTransport): Promise<PageMeta[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "ListPages"))
    .then((reply) => replyVariant<PageMeta[]>(reply, "PageList"));
