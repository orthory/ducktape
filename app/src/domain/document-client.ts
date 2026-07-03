// Typed client for the node's `document` module — the TS mirror of
// `crates/apps/document-interface`. A document is exactly an ordered list of
// blocks keyed by doc_id: there is NO authorship (submits take no origin) and
// NO "list docs" query — the store is keyed by sha256(doc_id) and cannot
// enumerate. Same contract as tasks-client/forge-client: camelCase params in,
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
