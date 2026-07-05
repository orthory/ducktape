// Typed client for the node's `document` module — the TS mirror of
// `crates/apps/document-interface`. A document is exactly an ordered list of
// blocks keyed by doc_id, and the module keeps a reserved INDEX entry so the
// store CAN enumerate its ids (ListDocs) — the browsable tree is derived from
// those "/"-delimited path ids. There is NO authorship (submits take no
// origin). Same contract as tasks-client/forge-client: camelCase params in,
// verbatim serde wire out, pure functions over an injected NodeTransport.

import type { BlockEvent, NodeTransport } from "./transport";
import { replyVariant } from "./wire";

// ── Wire types (Block records + DocReply payloads, verbatim) ─

export type BlockKind = "Paragraph" | "Heading" | "Code";

export interface Block {
  id: string;
  kind: BlockKind;
  text: string;
}

const TARGET = "document";

// ── Msgs (writes — one block op per submit; no origin) ──
//
// `after` positioning (same rule for InsertBlock and MoveBlock): null == "at
// the front" (index 0); a block id == "immediately after that block" (the
// anchor must exist, else the op errors).

export const createDoc = (
  transport: NodeTransport,
  params: { docId: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, { CreateDoc: { doc_id: params.docId } });

export const insertBlock = (
  transport: NodeTransport,
  params: { docId: string; after: string | null; block: Block },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    InsertBlock: {
      doc_id: params.docId,
      after: params.after,
      block: params.block,
    },
  });

export const updateBlock = (
  transport: NodeTransport,
  params: { docId: string; blockId: string; text: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    UpdateBlock: {
      doc_id: params.docId,
      block_id: params.blockId,
      text: params.text,
    },
  });

export const removeBlock = (
  transport: NodeTransport,
  params: { docId: string; blockId: string },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    RemoveBlock: { doc_id: params.docId, block_id: params.blockId },
  });

export const moveBlock = (
  transport: NodeTransport,
  params: { docId: string; blockId: string; after: string | null },
): Promise<BlockEvent> =>
  transport.submit(TARGET, {
    MoveBlock: {
      doc_id: params.docId,
      block_id: params.blockId,
      after: params.after,
    },
  });

// ── Queries (reads over committed state) ────────────────

/** The whole document as its ordered blocks, or null when the doc is absent. */
export const getDoc = (
  transport: NodeTransport,
  docId: string,
): Promise<Block[] | null> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, { GetDoc: { doc_id: docId } }))
    .then((reply) => replyVariant<Block[] | null>(reply, "Doc"));

/** A single block by id, or null when the doc or block is absent. */
export const getBlock = (
  transport: NodeTransport,
  params: { docId: string; blockId: string },
): Promise<Block | null> =>
  Promise.resolve()
    .then(() =>
      transport.query(TARGET, {
        GetBlock: { doc_id: params.docId, block_id: params.blockId },
      }),
    )
    .then((reply) => replyVariant<Block | null>(reply, "Block"));

/** Every known doc id, sorted — the module's enumeration index. The console
 *  derives its folder tree from these "/"-delimited path ids. */
export const listDocs = (transport: NodeTransport): Promise<string[]> =>
  Promise.resolve()
    .then(() => transport.query(TARGET, "ListDocs"))
    .then((reply) => replyVariant<string[]>(reply, "DocList"));

// ── Materialized view (the module's derived-index endpoint) ──

/** One search hit from document's materialized view (camelCase index wire). */
export interface DocSearchHit {
  docId: string;
  blockId: string;
  kind: BlockKind;
  text: string;
  height: number;
  time: number;
}

/** Full-text search over document blocks, newest first — served by the node's
 *  per-module index (document's own view endpoint), not canonical state. */
export const searchBlocks = (
  transport: NodeTransport,
  params: { text: string; docId?: string; limit?: number },
): Promise<DocSearchHit[]> =>
  Promise.resolve()
    .then(() =>
      transport.view(TARGET, {
        search: { text: params.text, docId: params.docId, limit: params.limit },
      }),
    )
    .then((reply) => replyVariant<DocSearchHit[]>(reply, "hits"));
